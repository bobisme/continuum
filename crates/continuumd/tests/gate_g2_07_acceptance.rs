//! G2-07 acceptance — **independent re-derivation** of "the prompt injection corpus cannot
//! trigger privileged operations" (bn-ugee0).
//!
//! > prompt injection corpus cannot trigger privileged operations.
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md:33`, gate G2, criterion **G2-07**
//!
//! The constitutional backdrop is INV-016 — content arriving from a source never carries
//! authority — and the delivering evidence is `crates/continuum-security/src/injection.rs`
//! (the corpus) plus `crates/continuumd/tests/g2_injection_corpus_evidence.rs` (twenty tests
//! reading the admission ledger).
//!
//! # What this file is, and what it deliberately is not
//!
//! It is **not** a re-run of the delivering suite. Re-running a suite and reading its green
//! is weaker than deriving the same fact by another route, and
//! `notes/plan/notes/PHASE_A_EXIT_PACKAGE.md` §5 says so itself. This file re-derives the
//! criterion from the **normative IDL** and from the **wire**, and shares no helper, no
//! fixture, no principal and no oracle with the delivering suite.
//!
//! Four method differences carry the independence.
//!
//! 1. **The privileged set is re-derived, not read off the registry.** The delivering
//!    suite's oracle is `registry::operation(name).has(Annotation::Privileged)` — the
//!    daemon's own transcription of the IDL. [`idl_scan`] is a second reader over
//!    `notes/plan/schemas/continuumd-native-protocol.idl` written to a different strategy
//!    from `tests/idl_conformance.rs`'s token-stream parser: it is line-oriented, anchored
//!    at column zero, and it recovers the *authority ladder itself* from the IDL's
//!    `enum AuthorityLevel` declaration order rather than from any Rust type.
//! 2. **The admission decision is predicted before it is observed.** [`predict`] computes,
//!    from the IDL text alone, whether each of the 45 corpus cases is admitted for each of
//!    five principals — a 225-cell matrix — and
//!    [`the_admission_matrix_is_predicted_from_the_idl_alone`] then confirms every cell at
//!    the wire. The delivering suite reads the ledger and asserts a property of what it
//!    finds; this file states what the ledger must contain before running anything.
//! 3. **The principal set is the whole ladder, including its top rung.** The delivering
//!    suite runs two principals: `cap_agent` at `propose` and `cap_reviser` at
//!    `revise-intent`. This file runs one capability at *every* rung, including
//!    [`PROMOTE`] — `promote`, the highest level this protocol can express, holding every
//!    data grant its parent holds, restricted by nothing except an empty
//!    `privileged_operations` list. That principal is admitted for **every** landed
//!    non-privileged operation the corpus drives at, which is what turns "the corpus was
//!    refused" into "the corpus was refused *by T3 and by nothing else*" — the ladder,
//!    scope, expiry, delegation and data-grant terms are all positively satisfied and
//!    therefore excluded as explanations.
//! 4. **Every request names live state.** The delivering suite's planter parses a payload
//!    as a handle where it can (`IntentHandle::new(payload).unwrap_or_else(…)`), so a
//!    `predictable handles` case is a request against a record that does not exist.
//!    [`plant`] never does that: every frame this file sends names the fixture's real
//!    proposal, real staged commitment and real evidence node, so no refusal here can be an
//!    accident of a dangling reference.
//!
//! # Findings, stated before any pass
//!
//! **F1 — the corpus drives at three of the five privileged operations this protocol
//! declares.** `repair.promote` and `repair.reject` are `@privileged`, and they are the
//! *only* two operations in the whole IDL at the ladder's top rung `promote`. No corpus case
//! names either. This is invisible in the delivering suite, whose coverage guard
//! `the_corpus_drives_at_every_privileged_operation_this_daemon_serves` filters on `is_landed`
//! and so compares against three. The gap is bounded and self-closing — that guard goes red
//! the day the repair family lands — but until then the criterion is evidenced over 3/5 of
//! the declared privileged surface, not 5/5.
//! [`the_corpus_drives_at_three_of_the_five_privileged_operations_this_protocol_declares`]
//! pins it, names the two, and fails when either lands without a case. Tracked `bn-1n7hy`.
//!
//! **F2 — under the delivering suite's default principal, most of the corpus never reaches a
//! handler.** `cap_agent` sits at `propose`. Sixteen of the nineteen operations the corpus
//! drives at require more than that, so of the 45 cases exactly **8** are admitted, **26**
//! are denied at T1 on the level ladder, and **11** never reach admission at all. Five of the
//! delivering suite's tests run `AGENT` alone — including its content-blindness differential
//! — so those comparisons are, for 37 of 45 cases, between two refusals produced before any
//! handler read a byte. The privilege bit is the operative term for exactly **10** of the 225
//! decisions in this file's matrix, and
//! [`the_delivering_suites_default_principal_reaches_a_handler_for_eight_of_forty_five_cases`]
//! derives that accounting mechanically rather than asserting it.
//!
//! **F3 — the delivering suite's headline claim is oracle-dependent on the daemon's own
//! transcription.** `no_case_in_the_corpus_triggers_a_privileged_operation` filters the
//! admission ledger with `registry::operation(name).has(Annotation::Privileged)` — the same
//! table the admission predicate consults. Drop `Annotation::Privileged` from `intent.lock`
//! in `protocol/registry.rs` and the daemon *admits* the corpus's three `intent.lock` cases
//! for an unprivileged capability, while that test still passes: the operation simply leaves
//! its filter. Measured, not argued — under that mutant nineteen of the delivering suite's
//! twenty tests are green and the one that fails is an arity check
//! (`operations.len() == 3`), not a claim about admission. Seven tests in this file fail on
//! it, because the set they compare against is read from the normative IDL rather than from
//! the transcription. That is the whole reason a re-derivation is worth more than a re-run,
//! and it is what [`the_privileged_set_re_derived_from_the_idl_is_exactly_the_delivering_oracles_set`]
//! and [`the_admission_matrix_is_predicted_from_the_idl_alone`] exist for. `idl_conformance.rs`
//! independently guards the same transcription by a third route; the point here is that
//! G2-07's own evidence did not.
//!
//! **F4 — nine of the 34 landed cases cannot carry their payload into a well-formed body.**
//! `evidence.verify`, `task.resume` and `task.cancel` declare no string or opaque field: their
//! only caller-supplied value is a handle. A payload can ride in only by *becoming* the
//! handle, which is what the delivering suite does and what costs the request its reference to
//! real state. [`payload_reaches_the_body`] records the split — 25 cases carry, 9 do not — so
//! the payload-independence differential below is honest about its own domain.
//!
//! # The mutants this file was proved against
//!
//! Each was applied to current trunk, run, and reverted; none is committed.
//!
//! | Mutant | Site | Caught by |
//! |---|---|---|
//! | the T3 privilege branch never fires | `daemon/admission.rs` | 5 tests, including the wire effect oracle |
//! | the T1 ladder comparison never fires | `daemon/admission.rs` | 4 tests |
//! | admission grants on a payload marker | `daemon/admission.rs` | 6 tests, including the payload differential |
//! | a privileged marker dropped in transcription | `protocol/registry.rs` | 7 tests (F3) |
//!
//! # Verdicts this file supports
//!
//! | Claim | Verdict | Evidence here |
//! |---|---|---|
//! | the privileged set is exactly the IDL's five | **SUPPORTED** | [`the_privileged_set_re_derived_from_the_idl_is_exactly_the_delivering_oracles_set`] |
//! | every authority marker agrees with the registry | **SUPPORTED** | [`every_authority_marker_the_idl_declares_agrees_with_the_registry`] |
//! | no corpus case triggers a privileged operation, at any rung | **SUPPORTED** | [`no_privileged_operation_is_admitted_at_any_rung_of_the_ladder`] |
//! | the refusal is T3 and not the ladder | **SUPPORTED** | [`the_refusal_at_the_top_rung_is_the_privilege_bit_alone`] |
//! | admission carries no payload term | **SUPPORTED** | [`the_admission_ledger_does_not_move_with_the_payload`] |
//! | no status a privileged reader can see on the wire moves | **SUPPORTED** | [`the_corpus_moves_no_status_a_privileged_reader_can_see_on_the_wire`] |
//! | coverage of the declared privileged surface | **PARTIAL — 3 of 5** (F1) | [`the_corpus_drives_at_three_of_the_five_privileged_operations_this_protocol_declares`] |
//! | the privileged set is checked against the normative file, not the transcription | **SUPPORTED here, absent from the delivering suite** (F3) | [`the_privileged_set_re_derived_from_the_idl_is_exactly_the_delivering_oracles_set`] |
//! | negative controls | all fire | [`the_scan_is_not_vacuous`], [`a_predictor_that_ignores_the_privilege_bit_disagrees_with_the_daemon`], [`a_predictor_that_ignores_the_ladder_disagrees_with_the_daemon`], [`the_wire_oracle_sees_a_real_privileged_effect`] |
//!
//! # Scope this file does not claim
//!
//! The seven corpus operations with no family in this process are refused by the codec, and
//! that is an accounting of absence rather than a defence — the delivering suite's `unlanded`
//! module already says so and this file does not restate it. Nor does this file clear plan
//! §24.5's ratified promotion gate, which additionally binds a run to a build and dependency
//! epoch and re-locks on any later success.

use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::canonical_json::Json as ContractJson;
use continuum_intent::contract::IntentContract;
use continuum_security::injection::CASES;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::snapshot::WorkspacePath;

use continuumd::codec::json::Json as WireJson;
use continuumd::codec::{self, CodecError};
use continuumd::daemon::Daemon;
use continuumd::daemon::context::ContextFamily;
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::Arguments;
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::VerificationFamily;
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope, ResultEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::ContextExpandRequest;
use continuumd::protocol::operations::evidence::{
    EvidenceLinkRequest, EvidenceQueryRequest, EvidenceVerifyRequest,
};
use continuumd::protocol::operations::intent::{
    IntentAcceptRequest, IntentGetRequest, IntentLockRequest, IntentProposeRevisionRequest,
    IntentRejectRequest,
};
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::operations::task::{TaskCancelRequest, TaskResumeRequest};
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, ContextHandle, ContinuationHandle, EpochIdentity,
    EvidenceHandle, IntentHandle, Opaque, OperationName, ProtocolVersion, RequestId, TaskHandle,
    Timestamp,
};
use continuumd::protocol::shared::{EvidenceQuery, IntentChangeSet, Target};
use continuumd::protocol::spec::{Annotation, Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, Encoding, ErrorCode, ExpansionRelation, Portfolio, ResultStatus,
    TargetKind,
};
use continuumd::transport::{self, Server};

/// The normative wire authority. "Where the prose and this file disagree, this file decides."
const IDL_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../notes/plan/schemas/continuumd-native-protocol.idl"
);

/// The Die Hard contract, the fixture's one registered proposal.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// The benign twin every payload-independence comparison runs against. Deliberately a value
/// with no verb, no operation name, no JSON and no imperative in it.
const BENIGN: &str = "a value with nothing in it but seventeen ordinary characters";

// =====================================================================================
// A second reader over the IDL
// =====================================================================================

mod idl_scan {
    //! A line-oriented scanner over the normative IDL, written for this file alone.
    //!
    //! `tests/idl_conformance.rs` already parses this file, from the EBNF in its own header,
    //! into a token stream. This scanner shares nothing with it on purpose: docs/03 §8 asks
    //! independent paths to differ in their parser, and a re-derivation that borrowed the
    //! first reader would be re-reading one transcription twice.
    //!
    //! The strategy is deliberately blunt, and its bluntness is what makes it safe. Only
    //! lines at **column zero** are structural, because the IDL declares every operation,
    //! enum and annotation run at the left margin and indents everything else. Doc prose,
    //! `rule` bodies and field declarations are therefore invisible to it by construction
    //! rather than by a skip list — which matters here, because the word `@privileged`
    //! appears six times in this file's prose and would otherwise be read as an annotation.
    //!
    //! Anything the scanner cannot account for is a panic naming the line. A scanner that
    //! silently skipped a construct could lose a whole privileged operation.

    use std::collections::BTreeSet;

    /// One `operation` declaration, as the IDL states it.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Declared {
        /// `namespace.verb`.
        pub name: String,
        /// The annotation run immediately above the declaration, without the `@`.
        pub annotations: BTreeSet<String>,
        /// The `authority` clause's identifier, as the IDL spells it (`revise_intent`).
        pub authority: String,
    }

    impl Declared {
        /// Whether the IDL marks this operation `@privileged`.
        #[must_use]
        pub fn is_privileged(&self) -> bool {
            self.annotations.contains("privileged")
        }
    }

    /// Every `operation` the IDL declares, in declaration order.
    ///
    /// # Panics
    ///
    /// On an operation with no `authority` clause, or a declaration whose braces never close.
    #[must_use]
    pub fn operations(source: &str) -> Vec<Declared> {
        let lines: Vec<&str> = source.lines().collect();
        let mut found = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let Some(rest) = line.strip_prefix("operation ") else {
                continue;
            };
            let Some(name) = rest.strip_suffix(" {") else {
                panic!(
                    "line {}: `operation` without an opening brace: {line:?}",
                    index + 1
                );
            };
            found.push(Declared {
                name: name.to_owned(),
                annotations: annotations_above(&lines, index),
                authority: authority_within(&lines, index),
            });
        }
        found
    }

    /// The contiguous run of column-zero annotation lines immediately above `index`.
    fn annotations_above(lines: &[&str], index: usize) -> BTreeSet<String> {
        let mut annotations = BTreeSet::new();
        let mut cursor = index;
        while cursor > 0 && lines[cursor - 1].starts_with('@') {
            cursor -= 1;
            for token in lines[cursor].split_whitespace() {
                let token = token
                    .strip_prefix('@')
                    .unwrap_or_else(|| panic!("line {}: not an annotation: {token:?}", cursor + 1));
                // `@since("3.4")` and friends carry an argument; the marker is the name.
                let name = token.split('(').next().unwrap_or(token);
                annotations.insert(name.to_owned());
            }
        }
        annotations
    }

    /// The `authority` clause inside the declaration opened at `index`.
    fn authority_within(lines: &[&str], index: usize) -> String {
        let mut depth = 0_i32;
        for (offset, line) in lines.iter().enumerate().skip(index) {
            depth += i32::try_from(line.matches('{').count()).expect("a short line");
            depth -= i32::try_from(line.matches('}').count()).expect("a short line");
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("authority ")
                && let Some(level) = rest.strip_suffix(';')
            {
                return level.to_owned();
            }
            if depth == 0 && offset > index {
                break;
            }
        }
        panic!(
            "the operation declared at line {} states no authority",
            index + 1
        )
    }

    /// The authority ladder, read off the IDL's own `enum AuthorityLevel` in declaration
    /// order — lowest rung first.
    ///
    /// RFC 0027's "Landed vocabulary" clause is explicit that the declaration order *is* the
    /// ladder, so recovering it from the text rather than from a Rust `Ord` keeps the whole
    /// prediction below sourced in the normative file.
    ///
    /// # Panics
    ///
    /// If the enum is absent or never closes.
    #[must_use]
    pub fn authority_ladder(source: &str) -> Vec<String> {
        let mut ladder = Vec::new();
        let mut inside = false;
        for line in source.lines() {
            if line == "enum AuthorityLevel {" {
                inside = true;
                continue;
            }
            if !inside {
                continue;
            }
            if line == "}" {
                return ladder;
            }
            let member = line.trim().trim_end_matches(',');
            // `revise_intent = "revise-intent"` — the identifier is what an `authority`
            // clause spells, so the wire token is not what this ladder is keyed on.
            let identifier = member.split('=').next().unwrap_or(member).trim();
            if identifier.is_empty() || identifier.starts_with("///") {
                continue;
            }
            ladder.push(identifier.to_owned());
        }
        panic!("`enum AuthorityLevel` never closes")
    }
}

/// The IDL text, read once per test that needs it.
fn idl_source() -> String {
    std::fs::read_to_string(IDL_PATH).expect("the normative IDL is in the tree")
}

/// The scanned operations, keyed by name.
fn declared() -> BTreeMap<String, idl_scan::Declared> {
    idl_scan::operations(&idl_source())
        .into_iter()
        .map(|operation| (operation.name.clone(), operation))
        .collect()
}

/// The rung `level` occupies on the IDL's ladder, lowest first.
fn rung(ladder: &[String], level: &str) -> usize {
    ladder
        .iter()
        .position(|member| member == level)
        .unwrap_or_else(|| panic!("{level:?} is not a rung of the ladder {ladder:?}"))
}

// =====================================================================================
// Principals — one per rung, each unprivileged
// =====================================================================================

/// One caller: an actor, the capability it presents, and the rung that capability sits on.
///
/// Every principal below carries an *explicitly present* profile with an empty
/// `privileged_operations` list and the one data grant this protocol declares. That is the
/// point of the design: the only thing that separates these five from the steward is the
/// privilege list, so a denial that survives at the top rung has exactly one explanation
/// left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Principal {
    actor: &'static str,
    capability: &'static str,
    level: AuthorityLevel,
    /// The rung's IDL identifier, so the prediction never leaves the IDL's vocabulary.
    idl_level: &'static str,
}

const READ: Principal = Principal {
    actor: "agent:rung-read",
    capability: "cap_rung_read",
    level: AuthorityLevel::Read,
    idl_level: "read",
};

/// The rung the delivering suite's `cap_agent` sits on.
const PROPOSE: Principal = Principal {
    actor: "agent:rung-propose",
    capability: "cap_rung_propose",
    level: AuthorityLevel::Propose,
    idl_level: "propose",
};

const EXECUTE: Principal = Principal {
    actor: "agent:rung-execute",
    capability: "cap_rung_execute",
    level: AuthorityLevel::Execute,
    idl_level: "execute",
};

/// The rung the delivering suite's `cap_reviser` sits on.
const REVISE: Principal = Principal {
    actor: "agent:rung-revise",
    capability: "cap_rung_revise",
    level: AuthorityLevel::ReviseIntent,
    idl_level: "revise_intent",
};

/// The strongest unprivileged capability this protocol can express: the ladder's top rung,
/// every data grant its parent holds, no scope restriction, and an empty privilege list.
const PROMOTE: Principal = Principal {
    actor: "agent:rung-promote",
    capability: "cap_rung_promote",
    level: AuthorityLevel::Promote,
    idl_level: "promote",
};

/// The whole ladder, lowest rung first.
const LADDER: [Principal; 5] = [READ, PROPOSE, EXECUTE, REVISE, PROMOTE];

/// The one privileged principal in this file, and it is a control rather than a subject: it
/// exists to show that the operations everyone above is denied are operations that *work*.
const STEWARD: Principal = Principal {
    actor: "human:steward",
    capability: "cap_steward",
    level: AuthorityLevel::ReviseIntent,
    idl_level: "revise_intent",
};

// =====================================================================================
// The fixture
// =====================================================================================

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 2)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

fn opname(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Null,
        proof: Nullable::Value(epoch("proof-1")),
        corpus: Nullable::Null,
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
}

fn profile(privileged: &[&str], grants: &[DataGrant]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| opname(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: grants.to_vec(),
        cross_principal_sharing: false,
    }
}

fn descriptor(
    handle: &str,
    actor: &str,
    level: AuthorityLevel,
    depth: u32,
    profile: Optional<CapabilityProfile>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: depth,
        profile,
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-g2-07-acceptance".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(
        &[ProtocolVersion::new(3, 1), version()],
        ProtocolWindow::new(3),
        ENCODINGS,
        &hello,
    )
    .expect("3.2 is served")
}

fn die_hard_contract() -> IntentContract {
    IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
}

/// The daemon behind the wire, plus the live references every frame this file sends names.
struct Fixture {
    server: Server,
    proposal: IntentHandle,
    evidence: EvidenceHandle,
    receipt: Commitment,
}

/// A daemon with every landed family, one capability at each rung of the ladder, a steward,
/// one *proposed* Die Hard contract, one staged file, and one appended evidence node.
///
/// Everything a frame can name exists. That is a deliberate difference from the delivering
/// suite's fixture: a request that names a record the daemon does not hold is refused for a
/// reason that has nothing to do with authority, and a re-derivation whose refusals could be
/// dangling references would prove nothing about the privilege bit.
fn fixture() -> Fixture {
    let root = Some(cap("cap_root"));
    let mut builder = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .now(now())
        .capability(
            descriptor(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                6,
                Optional::Present(profile(
                    &["intent.accept", "intent.reject", "intent.lock"],
                    &[DataGrant::ProductionTrace],
                )),
            ),
            None,
        )
        .capability(
            descriptor(
                STEWARD.capability,
                STEWARD.actor,
                STEWARD.level,
                4,
                Optional::Present(profile(
                    &["intent.accept", "intent.reject", "intent.lock"],
                    &[DataGrant::ProductionTrace],
                )),
            ),
            root.clone(),
        );

    // Provisioned at build time, not through `state_mut`: a capability registered after the
    // build "cannot *publish*, because the publication store's own registry is provisioned
    // once at build time", and a principal that could be admitted and then fail inside a
    // publisher would make the ledger and the effect disagree for a reason with nothing to
    // do with authority.
    for principal in LADDER {
        builder = builder.capability(
            descriptor(
                principal.capability,
                principal.actor,
                principal.level,
                4,
                // Present, and empty where privilege lives. `rule capability.profile_narrowing`
                // makes an empty list grant nothing rather than everything, so this is the
                // maximal unprivileged shape at each rung rather than a restricted one.
                Optional::Present(profile(&[], &[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        );
    }

    let mut daemon = builder
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(EvidenceFamily::new())
        .family(ObserveFamily)
        .family(VerificationFamily)
        .family(TaskFamily)
        .family(ContextFamily)
        .build();

    let contract = die_hard_contract();
    let proposal = intent_handle(&contract);
    daemon.state_mut().put_intent(
        proposal.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );

    let receipt = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("evidence/trace.json").expect("a workspace path"),
            b"{\"trace\":\"g2-07-acceptance\"}\n".to_vec(),
        )
        .expect("staging names its content");

    let mut fixture = Fixture {
        server: Server::new(daemon, negotiated()),
        proposal,
        evidence: EvidenceHandle::new("ev_g207placeholderaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .expect("an `ev_` handle"),
        receipt,
    };
    fixture.evidence = seed_evidence_node(&mut fixture);
    fixture
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    continuumd::daemon::identity::intent_to_wire(&stored).expect("an `in_` handle")
}

/// Append one evidence node through the producer path, so "no evidence status moved" is a
/// statement about a graph that has something in it.
///
/// The append goes through `observe.ingest` under the *steward*, not under any principal this
/// file measures: seeding through the wire keeps the node's identity the daemon's own, and
/// doing it under a principal outside the ladder keeps the seeding out of every ledger
/// comparison below.
fn seed_evidence_node(fixture: &mut Fixture) -> EvidenceHandle {
    let mut request = envelope("observe.ingest", STEWARD, "req_seed_evidence");
    request.arguments =
        transport::encode_arguments(&Arguments::ObserveIngest(ObserveIngestRequest {
            trace: fixture.receipt.clone(),
            instrumentation_profile: "seed".to_owned(),
        }))
        .expect("the body encodes");
    let bytes = codec::write_in::<WireJson, _>(&request).expect("the envelope encodes");
    let result = answer(fixture, &bytes);
    assert_eq!(
        result.status,
        ResultStatus::Ok,
        "the fixture's evidence node must exist, or every evidence-status claim below is \
         vacuous: {result:?}"
    );
    let (handle, _) = fixture
        .server
        .daemon()
        .state()
        .evidence_nodes()
        .next()
        .expect("the ingest appended a node");
    handle.clone()
}

// =====================================================================================
// One case, one frame
// =====================================================================================

fn budget() -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(32),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// The envelope one attempt travels in.
///
/// The idempotency and budget obligations are read off the registry's annotations rather than
/// decided per operation, because a frame that got them wrong is refused at step 6 for a
/// reason with nothing to do with authority — and step 6 runs *after* admission, so such a
/// frame would still corrupt the ledger comparison by succeeding where it should not.
fn envelope(operation: &str, principal: Principal, request_id: &str) -> RequestEnvelope {
    let spec = registry::operation(operation);
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request_id).expect("a well-formed request id"),
        idempotency_key: if spec.is_some_and(|spec| spec.has(Annotation::Mutation)) {
            Optional::Present(format!("idem-{request_id}"))
        } else {
            Optional::Absent
        },
        actor: who(principal.actor),
        capability: cap(principal.capability),
        operation: opname(operation),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: if spec.is_some_and(|spec| spec.has(Annotation::TaskStarting)) {
            Optional::Present(budget())
        } else {
            Optional::Absent
        },
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

/// A well-formed acceptance document whose `signature` carries `payload`.
fn acceptance(payload: &str) -> Opaque {
    let mut fields: BTreeMap<String, WireJson> = BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", payload),
        ("audit_record", "caller-supplied"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), WireJson::String(value.to_owned()));
    }
    Opaque::from_bytes(WireJson::Object(fields).to_canonical_bytes())
}

/// The change-set document a `intent.propose_revision` attempt carries.
///
/// A payload that is already a JSON object is used as itself, re-emitted through the canonical
/// writer because `Opaque` is canonical JSON on this wire; anything else is wrapped, because
/// the encoder refuses a non-object change set before the request is a request at all.
fn change_set(payload: &str) -> IntentChangeSet {
    let changes = match ContractJson::parse(payload.as_bytes()) {
        Ok(document @ ContractJson::Object(_)) => document.to_canonical_bytes(),
        _ => {
            let mut fields: BTreeMap<String, WireJson> = BTreeMap::new();
            fields.insert("rationale".to_owned(), WireJson::String(payload.to_owned()));
            WireJson::Object(fields).to_canonical_bytes()
        }
    };
    IntentChangeSet {
        changes: Opaque::from_bytes(changes),
        rationale: payload.to_owned(),
    }
}

/// Whether this operation's request body has a field a payload can ride in *without* the
/// request losing its reference to real state.
///
/// Three of the twelve landed corpus operations do not: `evidence.verify`, `task.resume` and
/// `task.cancel` declare a handle and nothing else. See F4.
fn payload_reaches_the_body(operation: &str) -> bool {
    !matches!(operation, "evidence.verify" | "task.resume" | "task.cancel")
}

/// The typed body an attempt carries, or [`None`] where this daemon serves no such operation.
///
/// Every handle is the fixture's own. Where the operation declares a caller-supplied string or
/// opaque document, the payload goes there; where it declares none, the request is the
/// well-formed one and [`payload_reaches_the_body`] records that the payload did not travel.
fn plant(operation: &str, payload: &str, fixture: &Fixture) -> Option<Arguments> {
    Some(match operation {
        "intent.accept" => Arguments::IntentAccept(IntentAcceptRequest {
            proposal: fixture.proposal.clone(),
            acceptance: acceptance(payload),
            bundle: Optional::Absent,
        }),
        "intent.reject" => Arguments::IntentReject(IntentRejectRequest {
            proposal: fixture.proposal.clone(),
            reason: payload.to_owned(),
        }),
        "intent.lock" => Arguments::IntentLock(IntentLockRequest {
            intent: fixture.proposal.clone(),
            policy: BTreeMap::from([("properties".to_owned(), payload.to_owned())]),
        }),
        "intent.propose_revision" => {
            Arguments::IntentProposeRevision(IntentProposeRevisionRequest {
                base: fixture.proposal.clone(),
                changes: change_set(payload),
            })
        }
        "intent.get" => Arguments::IntentGet(IntentGetRequest {
            intent: fixture.proposal.clone(),
        }),
        "evidence.verify" => Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence: fixture.evidence.clone(),
            expected_status: Optional::Absent,
        }),
        "evidence.link" => Arguments::EvidenceLink(EvidenceLinkRequest {
            subject: fixture.evidence.clone(),
            receipt: fixture.receipt.clone(),
            checker_profile: payload.to_owned(),
        }),
        "evidence.query" => Arguments::EvidenceQuery(EvidenceQueryRequest {
            query: EvidenceQuery {
                node_kinds: Optional::Absent,
                edge_kinds: Optional::Absent,
                statuses: Optional::Absent,
                claim_id: Optional::Present(payload.to_owned()),
                roots: Optional::Absent,
                max_depth: Optional::Absent,
            },
        }),
        "observe.ingest" => Arguments::ObserveIngest(ObserveIngestRequest {
            trace: fixture.receipt.clone(),
            instrumentation_profile: payload.to_owned(),
        }),
        "context.expand" => Arguments::ContextExpand(ContextExpandRequest {
            context: ContextHandle::new("ctx_g207fixturepackaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .expect("a `ctx_` handle"),
            anchor: payload.to_owned(),
            relation: ExpansionRelation::SourceSpan,
            depth: Optional::Absent,
        }),
        "verification.start" => Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::AllClaims,
                id: payload.to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
        "task.resume" => Arguments::TaskResume(TaskResumeRequest {
            continuation: ContinuationHandle::new(
                "cont_g207fixtureaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .expect("a `cont_` handle"),
            budget: Optional::Absent,
        }),
        "task.cancel" => Arguments::TaskCancel(TaskCancelRequest {
            task: TaskHandle::new("task_g207fixtureaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .expect("a `task_` handle"),
        }),
        _ => return None,
    })
}

/// The bytes one attempt puts on the wire.
fn frame(
    operation: &str,
    payload: &str,
    principal: Principal,
    request_id: &str,
    fixture: &Fixture,
) -> Vec<u8> {
    let mut request = envelope(operation, principal, request_id);
    request.arguments = match plant(operation, payload, fixture) {
        Some(arguments) => transport::encode_arguments(&arguments).expect("the body encodes"),
        None => Opaque::from_bytes(WireJson::String(payload.to_owned()).to_canonical_bytes()),
    };
    codec::write_in::<WireJson, _>(&request).expect("the envelope encodes")
}

fn answer(fixture: &mut Fixture, frame: &[u8]) -> ResultEnvelope {
    let bytes = fixture.server.answer(frame).expect("a result is encodable");
    codec::read_in::<WireJson, ResultEnvelope>(&bytes).expect("the result decodes")
}

fn error_code(result: &ResultEnvelope) -> Option<ErrorCode> {
    result.error.value().map(|error| error.code)
}

/// Whether this daemon resolves a request shape for `operation`.
///
/// Asked of the codec rather than of a list, so an operation that lands later moves this
/// answer without an edit here.
fn is_landed(operation: &str) -> bool {
    !matches!(
        codec::operations::decode_arguments(operation, &Opaque::from_bytes(b"{}".to_vec())),
        Err(CodecError::UnknownOperation)
    )
}

// =====================================================================================
// The prediction
// =====================================================================================

/// What the IDL says the admission decision must be, computed before any request is sent.
///
/// The predicate is the IDL's own two authority markers and nothing else:
///
/// ```text
/// admitted(operation, principal) =
///       this daemon resolves a shape for `operation`          (there is a request to admit)
///   and rung(principal) >= rung(authority(operation))          T1, the IDL's `authority` clause
///   and not privileged(operation)                              T3, the IDL's `@privileged`
/// ```
///
/// Every other term of RFC 0027's predicate is *positively satisfied* by construction and so
/// drops out: T2 is unrestricted (empty scope lists), T4 holds (each principal is a narrowing
/// child of the connection's own capability, unexpired and unrevoked), the one data grant the
/// protocol declares is held by every principal, and no operation is denied outright. That is
/// what makes a surviving denial attributable: with the other terms pinned true, `@privileged`
/// is the only bit left that can move the answer.
fn predict(
    operation: &str,
    principal: Principal,
    ladder: &[String],
    declared: &BTreeMap<String, idl_scan::Declared>,
) -> bool {
    let spec = declared
        .get(operation)
        .unwrap_or_else(|| panic!("the IDL declares no operation {operation:?}"));
    is_landed(operation)
        && rung(ladder, principal.idl_level) >= rung(ladder, &spec.authority)
        && !spec.is_privileged()
}

/// One observed decision.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Decision {
    case: &'static str,
    operation: String,
    principal: &'static str,
    /// `None` where the attempt never reached admission at all.
    admitted: Option<bool>,
    code: Option<ErrorCode>,
}

/// Run the whole corpus under one principal and report the decision for every case.
///
/// The admission ledger is read *positionally*: the daemon appends one record per attempt that
/// reaches admission, so walking the ledger forward alongside the corpus attributes each record
/// to the case that produced it without trusting any field of the record to identify it.
fn run_corpus(
    fixture: &mut Fixture,
    principal: Principal,
    payload_of: impl Fn(&str) -> String,
) -> Vec<Decision> {
    let mut decisions = Vec::new();
    // The ledger is append-only and the fixture's own seeding — and any wire read taken before
    // this call — already sits in it, so the walk starts where the corpus starts rather than at
    // zero. Nothing this file measures is ever read from a record it did not produce.
    let mut consumed = fixture.server.daemon().state().admissions().len();
    for (index, case) in CASES.iter().enumerate() {
        let payload = payload_of(case.payload);
        let bytes = frame(
            case.operation,
            &payload,
            principal,
            &format!("req_{}_{index}", principal.capability),
            fixture,
        );
        let result = answer(fixture, &bytes);
        let ledger = fixture.server.daemon().state().admissions();
        let admitted = if ledger.len() > consumed {
            assert_eq!(
                ledger.len(),
                consumed + 1,
                "{} produced more than one admission record",
                case.id
            );
            let record = &ledger[consumed];
            assert_eq!(
                record.operation, case.operation,
                "the ledger's next record is not this case's"
            );
            consumed += 1;
            Some(record.admitted)
        } else {
            None
        };
        decisions.push(Decision {
            case: case.id,
            operation: case.operation.to_owned(),
            principal: principal.capability,
            admitted,
            code: error_code(&result),
        });
    }
    decisions
}

/// The whole matrix: every case, at every rung.
fn matrix(payload_of: impl Fn(&str) -> String + Copy) -> Vec<Decision> {
    LADDER
        .into_iter()
        .flat_map(|principal| {
            let mut fixture = fixture();
            run_corpus(&mut fixture, principal, payload_of)
        })
        .collect()
}

fn hostile(payload: &str) -> String {
    payload.to_owned()
}

fn benign(_payload: &str) -> String {
    BENIGN.to_owned()
}

// =====================================================================================
// Leg 1 — the privileged set, re-derived
// =====================================================================================

#[test]
fn the_privileged_set_re_derived_from_the_idl_is_exactly_the_delivering_oracles_set() {
    // The delivering suite's oracle is `registry::operation(name).has(Annotation::Privileged)`
    // — the daemon's transcription. This compares it against a second reading of the normative
    // file, in both directions and over every operation, so a privileged marker the
    // transcription dropped and one it invented are both failures.
    let declared = declared();
    assert_eq!(
        declared.len(),
        registry::OPERATIONS.len(),
        "the scanner and the registry disagree about how many operations exist"
    );

    let from_idl: BTreeSet<&str> = declared
        .values()
        .filter(|operation| operation.is_privileged())
        .map(|operation| operation.name.as_str())
        .collect();
    let from_registry: BTreeSet<&str> = registry::OPERATIONS
        .iter()
        .filter(|spec| spec.has(Annotation::Privileged))
        .map(|spec| spec.name)
        .collect();
    assert_eq!(
        from_idl, from_registry,
        "the privileged set the IDL declares is not the set the delivering evidence tests"
    );

    // Named, so a reader of this file can check the set against the dossier without running
    // anything, and so that a change to it is a diff here.
    assert_eq!(
        from_idl.iter().copied().collect::<Vec<_>>(),
        vec![
            "intent.accept",
            "intent.lock",
            "intent.reject",
            "repair.promote",
            "repair.reject",
        ]
    );
}

#[test]
fn every_authority_marker_the_idl_declares_agrees_with_the_registry() {
    // `@privileged` is not the only authority marker in this file. The `authority` clause is
    // the other half of the admission predicate, and `@audit_recorded` is what plan §18.5
    // binds a privileged call to. All three are compared here, for all seventy-five
    // operations, so the prediction below rests on a checked reading rather than a believed
    // one.
    let ladder = idl_scan::authority_ladder(&idl_source());
    assert_eq!(
        ladder,
        vec!["read", "propose", "execute", "revise_intent", "promote"],
        "the ladder this file predicts with is not the ladder the IDL declares"
    );

    let declared = declared();
    let mut disagreements = Vec::new();
    for spec in registry::OPERATIONS {
        let Some(operation) = declared.get(spec.name) else {
            disagreements.push(format!("{}: the IDL declares no such operation", spec.name));
            continue;
        };
        if spec.authority.ident() != operation.authority {
            disagreements.push(format!(
                "{}: authority {:?} != IDL {:?}",
                spec.name,
                spec.authority.ident(),
                operation.authority
            ));
        }
        for annotation in [
            Annotation::Readonly,
            Annotation::Mutation,
            Annotation::TaskStarting,
            Annotation::Privileged,
            Annotation::AuditRecorded,
            Annotation::Paginated,
            Annotation::Streaming,
        ] {
            let held = spec.has(annotation);
            let stated = operation.annotations.contains(annotation.as_idl());
            if held != stated {
                disagreements.push(format!(
                    "{}: registry says {}@{} and the IDL says {}",
                    spec.name,
                    if held { "" } else { "no " },
                    annotation.as_idl(),
                    stated
                ));
            }
        }
    }
    assert!(
        disagreements.is_empty(),
        "the registry and the IDL disagree: {disagreements:#?}"
    );
}

#[test]
fn the_privileged_marker_is_a_genuine_restriction_of_the_operation_set() {
    // If every operation were privileged, or if `@privileged` were a synonym for a marker the
    // daemon checks elsewhere, the criterion would be about a different set than it appears to
    // be. Four facts, each read off the scan.
    let ladder = idl_scan::authority_ladder(&idl_source());
    let declared = declared();
    let privileged: Vec<&idl_scan::Declared> = declared
        .values()
        .filter(|operation| operation.is_privileged())
        .collect();

    // 1. Five of seventy-five. A strict, small minority.
    assert_eq!(privileged.len(), 5);
    assert_eq!(declared.len(), 75);

    // 2. Privilege implies audit-recording — the IDL's own legend says "@privileged
    //    privileged operation; audit-recorded" — and the converse fails, so the two markers
    //    are not one marker under two names.
    assert!(
        privileged
            .iter()
            .all(|operation| operation.annotations.contains("audit_recorded")),
        "the legend's implication does not hold in the declarations"
    );
    let audit_recorded = declared
        .values()
        .filter(|operation| operation.annotations.contains("audit_recorded"))
        .count();
    assert_eq!(audit_recorded, 7);

    // 3. Every privileged operation is a `@mutation`. A privileged read would be a different
    //    kind of thing and the corpus would need a different probe for it.
    assert!(
        privileged
            .iter()
            .all(|operation| operation.annotations.contains("mutation"))
    );

    // 4. Privilege is not the ladder. `intent.propose_revision` sits at `revise_intent` — the
    //    same rung as `intent.accept` — and is not privileged, which is the witness that the
    //    two markers are independent. And every operation at the ladder's *top* rung is
    //    privileged, which is why a principal at that rung is the sharpest instrument this
    //    protocol admits.
    let propose_revision = &declared["intent.propose_revision"];
    assert_eq!(
        propose_revision.authority,
        declared["intent.accept"].authority
    );
    assert!(!propose_revision.is_privileged());
    let top = ladder.last().expect("the ladder has a top rung");
    let at_top: Vec<&str> = declared
        .values()
        .filter(|operation| &operation.authority == top)
        .map(|operation| operation.name.as_str())
        .collect();
    assert_eq!(at_top, vec!["repair.promote", "repair.reject"]);
    assert!(at_top.iter().all(|name| declared[*name].is_privileged()));
}

#[test]
fn the_scan_is_not_vacuous() {
    // A reader that returned the same set whatever the file said would make every comparison
    // above a tautology. Three doctored texts, three different answers.
    let source = idl_source();
    let baseline: BTreeSet<String> = idl_scan::operations(&source)
        .into_iter()
        .filter(idl_scan::Declared::is_privileged)
        .map(|operation| operation.name)
        .collect();
    assert_eq!(baseline.len(), 5);

    let stripped = source.replace(
        "@mutation @privileged @audit_recorded\noperation intent.accept {",
        "@mutation @audit_recorded\noperation intent.accept {",
    );
    assert_ne!(stripped, source, "the doctored text must differ");
    let after: BTreeSet<String> = idl_scan::operations(&stripped)
        .into_iter()
        .filter(idl_scan::Declared::is_privileged)
        .map(|operation| operation.name)
        .collect();
    assert!(
        !after.contains("intent.accept") && after.len() == 4,
        "removing a marker did not move the derived set"
    );

    let added = source.replace(
        "@readonly @paginated\noperation evidence.query {",
        "@readonly @paginated @privileged\noperation evidence.query {",
    );
    assert_ne!(added, source, "the doctored text must differ");
    assert!(
        idl_scan::operations(&added)
            .into_iter()
            .filter(idl_scan::Declared::is_privileged)
            .any(|operation| operation.name == "evidence.query"),
        "adding a marker did not move the derived set"
    );

    let relevelled = source.replace(
        "operation intent.accept {\n  authority revise_intent;",
        "operation intent.accept {\n  authority promote;",
    );
    assert_ne!(relevelled, source, "the doctored text must differ");
    let moved = idl_scan::operations(&relevelled)
        .into_iter()
        .find(|operation| operation.name == "intent.accept")
        .expect("the operation is still declared");
    assert_eq!(moved.authority, "promote");
}

// =====================================================================================
// Leg 2 — what the corpus covers of that set (F1)
// =====================================================================================

#[test]
fn the_corpus_drives_at_three_of_the_five_privileged_operations_this_protocol_declares() {
    // F1. The delivering suite's coverage guard filters the privileged set on `is_landed`
    // before comparing, so it reports full coverage of three. This states the unfiltered fact.
    let declared = declared();
    let privileged: BTreeSet<&str> = declared
        .values()
        .filter(|operation| operation.is_privileged())
        .map(|operation| operation.name.as_str())
        .collect();
    let attempted: BTreeSet<&str> = CASES.iter().map(|case| case.operation).collect();

    let covered: Vec<&str> = privileged.intersection(&attempted).copied().collect();
    let uncovered: Vec<&str> = privileged.difference(&attempted).copied().collect();
    assert_eq!(
        covered,
        vec!["intent.accept", "intent.lock", "intent.reject"]
    );
    assert_eq!(uncovered, vec!["repair.promote", "repair.reject"]);

    // The gap is bounded by the reason for it: neither uncovered operation has a family in
    // this process, so neither is reachable today. This assertion is the tripwire — the day a
    // repair family lands, an uncovered privileged operation becomes a reachable one and this
    // test goes red before anything else notices.
    for operation in &uncovered {
        assert!(
            !is_landed(operation),
            "{operation} is privileged, landed, and no corpus case drives at it"
        );
    }

    // And the corpus does carry cases for seven *other* unlanded operations, so "unlanded"
    // does not explain the absence: the corpus's own forward-compatibility argument — a lane
    // that lands a surface inherits its escape attempt — simply does not extend to these two.
    let unlanded_but_carried: BTreeSet<&str> = attempted
        .iter()
        .copied()
        .filter(|operation| !is_landed(operation))
        .collect();
    assert_eq!(unlanded_but_carried.len(), 7);
}

// =====================================================================================
// Leg 3 — the admission matrix, predicted from the IDL
// =====================================================================================

#[test]
fn the_admission_matrix_is_predicted_from_the_idl_alone() {
    // The centrepiece. Forty-five cases at five rungs is 225 decisions, and every one of them
    // is computed from the normative text *before* the daemon is asked. A daemon whose
    // admission carried any term the IDL does not declare — a payload term above all — would
    // disagree in at least one cell.
    let ladder = idl_scan::authority_ladder(&idl_source());
    let declared = declared();
    let observed = matrix(hostile);
    assert_eq!(observed.len(), CASES.len() * LADDER.len());

    let mut disagreements = Vec::new();
    for decision in &observed {
        let principal = LADDER
            .into_iter()
            .find(|principal| principal.capability == decision.principal)
            .expect("every decision names a rung");
        let expected = predict(&decision.operation, principal, &ladder, &declared);
        let actual = decision.admitted.unwrap_or(false);
        if expected != actual {
            disagreements.push(format!(
                "{} at {}: the IDL predicts admitted={expected}, the daemon recorded {:?}",
                decision.case, decision.principal, decision.admitted
            ));
        }
        // An attempt that never reached admission must be one this daemon serves no shape
        // for; the converse — a landed operation that skipped the ledger — would be a hole.
        assert_eq!(
            decision.admitted.is_none(),
            !is_landed(&decision.operation),
            "{}: the ledger and the codec disagree about whether this operation exists",
            decision.case
        );
    }
    assert!(
        disagreements.is_empty(),
        "the daemon's admission is not the IDL's: {disagreements:#?}"
    );

    // The matrix is not trivially one-valued in either direction.
    let admitted = observed
        .iter()
        .filter(|decision| decision.admitted == Some(true))
        .count();
    let denied = observed
        .iter()
        .filter(|decision| decision.admitted == Some(false))
        .count();
    let absent = observed
        .iter()
        .filter(|decision| decision.admitted.is_none())
        .count();
    assert_eq!(
        absent,
        11 * LADDER.len(),
        "eleven unlanded cases at five rungs"
    );
    assert!(
        admitted > 0 && denied > 0,
        "{admitted} admitted, {denied} denied"
    );
    assert_eq!(admitted + denied + absent, CASES.len() * LADDER.len());
}

#[test]
fn no_privileged_operation_is_admitted_at_any_rung_of_the_ladder() {
    // G2-07 itself, at the widest principal scope this protocol can express, and read off the
    // admission ledger rather than off an error code — `CapabilityDenied` is also what a
    // handler answers for an artifact the daemon does not hold (RFC 0027 X2), so the wire code
    // alone cannot separate a refusal from a not-found.
    let declared = declared();
    let observed = matrix(hostile);

    let privileged: Vec<&Decision> = observed
        .iter()
        .filter(|decision| declared[&decision.operation].is_privileged())
        .collect();
    // Ten privileged attempts per rung — five `intent.accept`, three `intent.lock`, two
    // `intent.reject` — stated exactly, so a corpus that stopped attempting them would fail
    // here rather than pass vacuously.
    assert_eq!(privileged.len(), 10 * LADDER.len());

    for decision in privileged {
        assert_eq!(
            decision.admitted,
            Some(false),
            "{} was admitted at {} — a privileged operation the corpus triggered",
            decision.case,
            decision.principal
        );
        assert_eq!(
            decision.code,
            Some(ErrorCode::CapabilityDenied),
            "{} was refused at {}, but not by the capability check",
            decision.case,
            decision.principal
        );
    }
}

#[test]
fn the_refusal_at_the_top_rung_is_the_privilege_bit_alone() {
    // The claim the delivering suite makes with one comparison between two capabilities, made
    // here as a property of the whole corpus at the ladder's top.
    //
    // `PROMOTE` holds the highest level this protocol declares, every data grant its parent
    // holds, no scope restriction, no denied operation, no expiry, and a valid delegation to
    // the connection's own capability. So T1, T2, T4 and the data-grant clause are *positively
    // satisfied* for every operation in the corpus — and this test shows that by exhibiting
    // the admissions. What is left denied is denied by T3, by elimination rather than by
    // assertion.
    let declared = declared();
    let mut fixture = fixture();
    let decisions = run_corpus(&mut fixture, PROMOTE, hostile);

    let mut admitted = 0;
    let mut denied = 0;
    for decision in &decisions {
        let Some(recorded) = decision.admitted else {
            continue;
        };
        if declared[&decision.operation].is_privileged() {
            assert!(
                !recorded,
                "{}: privileged and admitted at the top rung",
                decision.case
            );
            denied += 1;
        } else {
            assert!(
                recorded,
                "{}: not privileged, and denied at the top rung — some term other than the \
                 privilege bit is deciding, and the elimination argument does not hold",
                decision.case
            );
            admitted += 1;
        }
    }
    assert_eq!(denied, 10, "the ten privileged attempts");
    assert_eq!(admitted, 24, "every other landed attempt");
    assert_eq!(admitted + denied, 34, "the corpus's landed cases");
}

#[test]
fn the_admission_ledger_does_not_move_with_the_payload() {
    // INV-016 as a measurement: content never carries authority. The same 225 attempts are run
    // with the corpus's hostile payloads and with one inert string, and the two ledgers must be
    // equal decision for decision. Anything that read a payload on the way to a decision — a
    // marker, a heuristic, a sanitizer that failed open — would separate them.
    let with_corpus = matrix(hostile);
    let with_benign = matrix(benign);
    assert_eq!(with_corpus.len(), with_benign.len());

    let mut moved = Vec::new();
    for (hostile, benign) in with_corpus.iter().zip(&with_benign) {
        assert_eq!(hostile.case, benign.case);
        if hostile.admitted != benign.admitted {
            moved.push(format!(
                "{} at {}: {:?} with the corpus payload, {:?} without it",
                hostile.case, hostile.principal, hostile.admitted, benign.admitted
            ));
        }
    }
    assert!(
        moved.is_empty(),
        "the admission decision moved with the payload: {moved:#?}"
    );

    // The differential has a domain, and F4 is what bounds it: nine of the thirty-four landed
    // cases carry no payload into their body at all, so for those the two runs are the same
    // request and the comparison is trivially true. Stated rather than hidden.
    let carrying = CASES
        .iter()
        .filter(|case| is_landed(case.operation) && payload_reaches_the_body(case.operation))
        .count();
    let inert = CASES
        .iter()
        .filter(|case| is_landed(case.operation) && !payload_reaches_the_body(case.operation))
        .count();
    assert_eq!(carrying, 25);
    assert_eq!(inert, 9);
    assert_eq!(carrying + inert, 34);
}

// =====================================================================================
// Leg 4 — the accounting the delivering suite's principals imply (F2)
// =====================================================================================

#[test]
fn the_delivering_suites_default_principal_reaches_a_handler_for_eight_of_forty_five_cases() {
    // F2. `cap_agent` — the delivering suite's default runner, and the only principal five of
    // its tests use — sits at `propose`. This derives, mechanically, how much of the corpus
    // that principal can actually put in front of a handler, and attributes every refusal it
    // gets to the term that produced it.
    let ladder = idl_scan::authority_ladder(&idl_source());
    let declared = declared();
    let mut fixture = fixture();
    let at_propose = run_corpus(&mut fixture, PROPOSE, hostile);

    let admitted: Vec<&Decision> = at_propose
        .iter()
        .filter(|decision| decision.admitted == Some(true))
        .collect();
    let denied: Vec<&Decision> = at_propose
        .iter()
        .filter(|decision| decision.admitted == Some(false))
        .collect();
    let never: Vec<&Decision> = at_propose
        .iter()
        .filter(|decision| decision.admitted.is_none())
        .collect();
    assert_eq!(admitted.len(), 8);
    assert_eq!(denied.len(), 26);
    assert_eq!(never.len(), 11);

    // Attribution. A case denied at `propose` whose operation is *not* privileged was refused
    // by the ladder, because the top rung admits it — which the previous test established by
    // exhibiting the admission. Sixteen of the twenty-six refusals are of that kind, and the
    // remaining ten are the privilege bit.
    let by_ladder = denied
        .iter()
        .filter(|decision| !declared[&decision.operation].is_privileged())
        .count();
    let by_privilege = denied
        .iter()
        .filter(|decision| declared[&decision.operation].is_privileged())
        .count();
    assert_eq!(by_ladder, 16);
    assert_eq!(by_privilege, 10);

    // The same fact from the IDL rather than from the run: sixteen of the nineteen operations
    // the corpus drives at are above `propose`.
    let above: Vec<&str> = CASES
        .iter()
        .map(|case| case.operation)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|operation| {
            rung(&ladder, &declared[*operation].authority) > rung(&ladder, PROPOSE.idl_level)
        })
        .collect();
    assert_eq!(above.len(), 16);

    // So of the 225 decisions in the matrix, the privilege bit is the operative term for
    // exactly fifty — ten cases at each of five rungs — and for the other 175 the answer is
    // decided by the ladder or by the absence of a family.
    assert_eq!(
        CASES
            .iter()
            .filter(|case| declared[case.operation].is_privileged())
            .count()
            * LADDER.len(),
        50
    );
}

// =====================================================================================
// Leg 5 — the effect, read through the wire
// =====================================================================================

/// What a privileged reader sees of the state a prohibited outcome would move.
///
/// Read through `Server::answer` rather than through `daemon().state()`. That is the
/// independence that matters here: the delivering suite's oracle and the daemon's ledger are
/// two views of one in-process structure, and an effect that moved the registry without moving
/// the ledger would be invisible to both. These bytes are what a client is told.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WireView {
    intent: Vec<u8>,
    evidence: Vec<u8>,
}

fn wire_view(fixture: &mut Fixture) -> WireView {
    WireView {
        intent: read_back(fixture, "intent.get", "req_view_intent"),
        evidence: read_back(fixture, "evidence.query", "req_view_evidence"),
    }
}

/// One read, under the steward, returning the answer's payload bytes.
fn read_back(fixture: &mut Fixture, operation: &str, request_id: &str) -> Vec<u8> {
    // A fresh request id each time: a read is `@readonly`, so it carries no idempotency key
    // and cannot be a replay, but a repeated id would still be a correlation collision.
    let unique = format!(
        "{request_id}_{}",
        fixture.server.daemon().state().admissions().len()
    );
    let bytes = frame(operation, "", STEWARD, &unique, fixture);
    let result = answer(fixture, &bytes);
    assert_eq!(
        result.status,
        ResultStatus::Ok,
        "the wire oracle must be able to read: {result:?}"
    );
    result
        .payload
        .value()
        .expect("a read answers with a payload")
        .as_bytes()
        .to_vec()
}

#[test]
fn the_corpus_moves_no_status_a_privileged_reader_can_see_on_the_wire() {
    // research/35's first two prohibited outcomes — an unprivileged intent-status alteration
    // and an unprivileged evidence-status alteration — measured as what a client is told,
    // before and after the corpus runs at the top rung. The top rung is the right place to
    // measure: it is the run in which the largest number of payloads reach a live handler.
    let mut fixture = fixture();
    let before = wire_view(&mut fixture);
    let decisions = run_corpus(&mut fixture, PROMOTE, hostile);
    assert_eq!(
        decisions
            .iter()
            .filter(|decision| decision.admitted == Some(true))
            .count(),
        24,
        "twenty-four payloads must actually reach a handler, or this comparison is about \
         nothing"
    );
    let after = wire_view(&mut fixture);
    assert_eq!(
        before, after,
        "the corpus moved state a privileged reader can see"
    );

    // The same fact in the daemon's own terms, as a second reading: no intent left the status
    // it was registered at, and no evidence node changed status.
    let statuses: Vec<&'static str> = fixture
        .server
        .daemon()
        .state()
        .intents()
        .map(|(_, record)| record.status.as_wire())
        .collect();
    assert_eq!(statuses, vec![RegistryStatus::Proposed.as_wire()]);
}

#[test]
fn the_wire_oracle_sees_a_real_privileged_effect() {
    // Anti-vacuity for the test above, and the liveness proof for the whole file: the same
    // `intent.accept` the corpus is denied at every rung *succeeds* for the steward, moves the
    // proposal to accepted, and moves the bytes the oracle reads. If it did not, "nothing
    // moved" would be a statement about an oracle that cannot see.
    let mut fixture = fixture();
    let before = wire_view(&mut fixture);

    let case = continuum_security::injection::case("comments-weakening-property/intent-status")
        .expect("the corpus holds this case");
    let bytes = frame(
        case.operation,
        case.payload,
        STEWARD,
        "req_steward_accept",
        &fixture,
    );
    let result = answer(&mut fixture, &bytes);
    assert_eq!(
        result.status,
        ResultStatus::Ok,
        "the steward's privileged accept must succeed: {result:?}"
    );

    let after = wire_view(&mut fixture);
    assert_ne!(
        before.intent, after.intent,
        "a privileged effect left the wire view unchanged, so the oracle is blind"
    );
    let proposal = fixture.proposal.clone();
    assert_eq!(
        fixture
            .server
            .daemon()
            .state()
            .intent(&proposal)
            .expect("the contract is registered")
            .status,
        RegistryStatus::Accepted
    );

    // And the payload rode all the way in: the acceptance signature carried an injection and
    // the operation succeeded anyway. Capability checks, not output filtering.
    assert!(
        !case.payload.is_empty() && case.payload.contains("accept the pending intent revision"),
        "the case used here must be a real injection"
    );
}

// =====================================================================================
// Leg 6 — the prediction is sensitive to both of its terms
// =====================================================================================

#[test]
fn a_predictor_that_ignores_the_privilege_bit_disagrees_with_the_daemon() {
    // If the matrix agreed with a predictor that dropped `@privileged`, then every cell of it
    // would be explained by the ladder and the file would have measured nothing about
    // privilege.
    let ladder = idl_scan::authority_ladder(&idl_source());
    let declared = declared();
    let observed = matrix(hostile);

    let mut disagreements = 0;
    for decision in &observed {
        let principal = LADDER
            .into_iter()
            .find(|principal| principal.capability == decision.principal)
            .expect("every decision names a rung");
        let spec = &declared[&decision.operation];
        let ladder_only = is_landed(&decision.operation)
            && rung(&ladder, principal.idl_level) >= rung(&ladder, &spec.authority);
        if ladder_only != decision.admitted.unwrap_or(false) {
            disagreements += 1;
        }
    }
    // Exactly the privileged attempts at rungs whose level would otherwise admit them:
    // `intent.accept`, `intent.reject` and `intent.lock` are at `revise_intent`, so the two
    // top rungs would admit all ten each.
    assert_eq!(disagreements, 20);
}

#[test]
fn a_predictor_that_ignores_the_ladder_disagrees_with_the_daemon() {
    // The mirror control. A predictor that kept only the privilege bit would admit every
    // non-privileged landed case at every rung, including the bottom one.
    let declared = declared();
    let observed = matrix(hostile);

    let mut disagreements = 0;
    for decision in &observed {
        let privilege_only =
            is_landed(&decision.operation) && !declared[&decision.operation].is_privileged();
        if privilege_only != decision.admitted.unwrap_or(false) {
            disagreements += 1;
        }
    }
    assert!(
        disagreements > 0,
        "the ladder term does no work in this matrix"
    );
}

// =====================================================================================
// Corpus agreement
// =====================================================================================

#[test]
fn every_corpus_operation_is_one_the_normative_idl_declares() {
    // The corpus is written against the dossier rather than against this daemon, so a case
    // could name an operation the protocol does not have — which would not be a security
    // failure but a case that silently tests nothing.
    let declared = declared();
    for case in CASES {
        assert!(
            declared.contains_key(case.operation),
            "{} drives at {:?}, which the IDL does not declare",
            case.id,
            case.operation
        );
    }
    assert_eq!(CASES.len(), 45);
}
