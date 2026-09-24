//! The signing wire (protocol 3.8, bn-3glnv, plan §18.6, ADR-0054, TEST-9-07).
//!
//! Every test here drives the real operations through `Daemon::dispatch`: the `signing`
//! family, `intent.export_bundle`, `intent.import_bundle`, `intent.accept` naming a bundle,
//! `evidence.link`, and `evidence.get`. The library (`continuum_evidence::signing`) is used
//! only to build hostile input and as the oracle a differential test compares against.
//!
//! | Property | Tests |
//! |---|---|
//! | authority: unscoped, privileged, version-gated | `every_signing_wire_operation_is_refused_to_a_scoped_grant`, `the_writes_are_refused_without_the_privilege`, `the_signing_wire_is_refused_below_protocol_3_8` |
//! | lifecycle: monotone and replay-safe | `mint_rotate_revoke_move_the_registry_forward_only`, `a_replayed_rotation_does_not_rotate_twice`, `rotation_needs_the_held_key`, `mint_without_an_entropy_capability_records_nothing` |
//! | no key material leaves the daemon | `no_answer_fault_or_debug_output_carries_key_material` |
//! | verification is total and fails closed | `signing_verify_is_total_and_typed`, `a_revoked_held_key_refuses_to_sign_anything` |
//! | receipts, packs, bundles signed and verified | `a_receipt_signature_travels_on_evidence_get_and_verifies_over_the_wire`, `a_domain_pack_signed_over_the_wire_verifies_and_a_tampered_one_does_not`, `the_daemons_bundle_signature_matches_the_library_signing_the_same_body` |
//! | intent.accept runs the CI check and the A1–A4 acceptance chain on a held bundle | `an_exported_bundle_imports_and_its_chain_accepts_the_proposal`, `accept_fails_closed_on_every_broken_chain`, `a_record_is_accepted_only_as_its_chain_signed_it`, `an_acceptance_by_a_rotated_or_revoked_signer_does_not_vouch`, `a_local_acceptance_at_3_8_records_a_verifiable_chain`, `a_signed_local_acceptance_names_only_the_admitted_principal_at_the_daemons_time` |
//! | revocation travels inside the bundle, attested by the keys it concerns | `a_revocation_carried_by_a_bundle_is_adopted_and_a_stale_bundle_cannot_resurrect_the_key`, `a_known_signer_learns_its_attested_rotation_and_revocation`, `a_signer_cannot_claim_a_known_or_unseen_pinned_key_without_its_signature`, `a_link_about_this_daemons_own_key_is_never_adopted`, `a_retired_key_adopts_no_facts`, `link_order_and_replay_do_not_change_what_is_adopted`, `a_loss_recovery_never_travels`, `a_bundle_whose_own_signature_fails_adopts_no_link`, `an_import_refused_for_quota_adopts_nothing`, `a_full_registry_can_still_revoke`, `a_held_key_revocation_always_leaves_room_for_its_replacement`, `a_bundle_that_rotates_one_key_twice_is_refused_whole`, `an_unpinned_successor_is_never_introduced`, `every_order_of_a_rotation_chain_gives_the_same_standing_and_facts`, `a_prelearned_first_key_ends_rotated_and_fails_ci_acceptance`, `a_cycle_or_a_shared_successor_is_refused_whole`, `an_install_that_would_leave_a_compromise_unrecoverable_is_refused` |
//! | identity collisions resolve by exact bytes (cr-2unxyh) | `an_import_whose_contract_collides_with_a_held_one_is_refused_before_anything_changes`, `accept_never_takes_a_signature_over_one_body_for_another_under_the_same_identity`, `a_bundle_identity_that_names_other_bytes_is_refused_on_import_and_export`, `a_receipt_node_identity_that_names_another_receipt_is_refused_before_signing`, `a_receipt_node_keeps_its_first_bytes_and_its_first_signature`, `staging_never_overwrites_content_under_a_colliding_commitment` |
//! | bounded, whole-or-nothing parsing | `truncated_extended_or_oversize_bundles_are_refused_whole`, `a_bundle_whose_contract_is_not_its_declared_identity_is_refused`, `import_is_idempotent_and_never_raises_status` |

use std::collections::BTreeMap;

use continuum_evidence::actor::ActorId as SigningActor;
use continuum_evidence::signing::{
    AllowedSigners, ArtifactSignature, EntropyUnavailable, KeyEntropy, SEED_LEN,
    SignedArtifactKind as Kind, SignerIdentity, SignerLink, SigningRegistry, Zeroizing,
};
use continuum_intent::canonical_json::Json as ContractJson;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::bundle::{
    BundleBody, BundleContract, MAX_BUNDLE_LEN, decode_signed, encode_signed, signed_bytes_identity,
};
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::signing::SigningFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest, ReceiptSigner};
use continuumd::protocol::envelope::{Budget, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::evidence::{EvidenceGetRequest, EvidenceLinkRequest};
use continuumd::protocol::operations::intent::{
    IntentAcceptRequest, IntentExportBundleRequest, IntentImportBundleRequest,
};
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::operations::signing::{
    SigningMintRequest, SigningRegistryRequest, SigningRevokeRequest, SigningRotateRequest,
    SigningSignPackRequest, SigningVerifyRequest,
};
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, IntentBundleHandle, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, SignerHandle, Timestamp,
};
use continuumd::protocol::spec::{Annotation, Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, Encoding, ErrorCode, RevocationReason, SignatureOutcome,
    SignedArtifactKind, StructuralOutcome,
};

const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

const STEWARD: (&str, &str) = ("human:steward", "cap_steward");
const SCOPED: (&str, &str) = ("human:scoped", "cap_scoped");
const REVISER: (&str, &str) = ("human:reviser", "cap_reviser");
const READER: (&str, &str) = ("agent:reader", "cap_reader");
const PRODUCER: (&str, &str) = ("agent:producer", "cap_producer");
const CHECKER: (&str, &str) = ("service:checker", "cap_checker");

/// The privileges the steward holds: every privileged write of the signing wire, and the
/// intent acceptance the bundle path ends in.
const PRIVILEGES: [&str; 8] = [
    "intent.accept",
    "intent.lock",
    "intent.export_bundle",
    "intent.import_bundle",
    "signing.mint",
    "signing.rotate",
    "signing.revoke",
    "signing.sign_pack",
];

const ACCEPTED_AT: &str = "2026-09-23T00:00:00.000Z";

// --- fixtures ------------------------------------------------------------------------------

/// Deterministic entropy: seed `[n; 32]`, then `[n + 1; 32]`, and so on.
struct Seeds(u8);

impl KeyEntropy for Seeds {
    fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
        self.0 = self.0.wrapping_add(1);
        Ok(Zeroizing::new([self.0; SEED_LEN]))
    }
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("capability")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("actor")
}

fn opname(operation: &str) -> OperationName {
    OperationName::new(operation).expect("operation")
}

fn profile(privileged: &[&str], grants: &[DataGrant]) -> Optional<CapabilityProfile> {
    Optional::Present(CapabilityProfile {
        privileged_operations: privileged.iter().map(|op| opname(op)).collect(),
        denied_operations: Vec::new(),
        data_grants: grants.to_vec(),
        cross_principal_sharing: false,
    })
}

fn grant(
    principal: (&str, &str),
    level: AuthorityLevel,
    profile: Optional<CapabilityProfile>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(principal.1),
        actor: who(principal.0),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 3,
        profile,
        instances: Optional::Absent,
    }
}

fn negotiated(version: ProtocolVersion) -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version,
            high: version,
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-daemon-signing".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version], ProtocolWindow::new(3), ENCODINGS, &hello).expect("served")
}

fn v38() -> ProtocolVersion {
    ProtocolVersion::new(3, 8)
}

/// One daemon: a root, the principals above, and the families the signing wire touches.
struct World {
    daemon: Daemon,
    version: ProtocolVersion,
    next: u32,
}

#[derive(Default)]
struct Setup {
    entropy: Option<u8>,
    allowed: Option<AllowedSigners>,
    signer: Option<ReceiptSigner>,
    version: Option<ProtocolVersion>,
    collide: Option<Colliding>,
}

fn world(setup: Setup) -> World {
    let version = setup.version.unwrap_or_else(v38);
    let root = Some(cap("cap_root"));
    let all: Vec<&str> = PRIVILEGES.to_vec();
    let mut scoped = grant(SCOPED, AuthorityLevel::Promote, profile(&all, &[]));
    scoped.intents = vec![IntentHandle::new("in_somewhere").expect("an in_")];
    let base = match setup.collide {
        Some(colliding) => Daemon::builder(colliding, negotiated(version), cap("cap_root")),
        None => Daemon::builder(Blake3Identity, negotiated(version), cap("cap_root")),
    };
    let mut builder = base
        .now(Timestamp::new("2026-09-23T00:00:00.000Z").expect("timestamp"))
        .capability(
            CapabilityDescriptor {
                delegation_depth: 5,
                ..grant(
                    ("service:continuumd", "cap_root"),
                    AuthorityLevel::Promote,
                    profile(&all, &[DataGrant::ProductionTrace]),
                )
            },
            None,
        )
        .capability(
            grant(STEWARD, AuthorityLevel::ReviseIntent, profile(&all, &[])),
            root.clone(),
        )
        .capability(scoped, root.clone())
        .capability(
            grant(REVISER, AuthorityLevel::ReviseIntent, profile(&[], &[])),
            root.clone(),
        )
        .capability(
            grant(READER, AuthorityLevel::Read, Optional::Absent),
            root.clone(),
        )
        .capability(
            grant(
                PRODUCER,
                AuthorityLevel::Execute,
                profile(&[], &[DataGrant::ProductionTrace]),
            ),
            root.clone(),
        )
        .capability(
            grant(CHECKER, AuthorityLevel::Promote, profile(&[], &[])),
            root,
        )
        .family(SigningFamily)
        .family(IntentFamily)
        .family(EvidenceFamily::verifying_as(who(CHECKER.0)))
        .family(ObserveFamily);
    if let Some(start) = setup.entropy {
        builder = builder.key_entropy(Box::new(Seeds(start)));
    }
    if let Some(allowed) = &setup.allowed {
        builder = builder.allowed_signers(allowed);
    }
    if let Some(signer) = setup.signer {
        builder = builder.receipt_signer(signer);
    }
    World {
        daemon: builder.build(),
        version,
        next: 0,
    }
}

fn with_entropy(start: u8) -> World {
    world(Setup {
        entropy: Some(start),
        ..Setup::default()
    })
}

impl World {
    fn call(&mut self, principal: (&str, &str), arguments: Arguments) -> OperationOutcome {
        self.next += 1;
        let key = format!("k{}", self.next);
        self.call_keyed(principal, arguments, &key)
    }

    fn call_keyed(
        &mut self,
        principal: (&str, &str),
        arguments: Arguments,
        key: &str,
    ) -> OperationOutcome {
        let operation = arguments.operation();
        let spec = registry::operation(operation).expect("a declared operation");
        let mutation = spec.has(Annotation::Mutation);
        self.next += 1;
        let envelope = RequestEnvelope {
            protocol_version: self.version,
            request_id: RequestId::new(&format!("req_{}", self.next)).expect("request id"),
            idempotency_key: if mutation {
                Optional::Present(key.to_owned())
            } else {
                Optional::Absent
            },
            actor: who(principal.0),
            capability: cap(principal.1),
            operation: opname(operation),
            snapshot: Nullable::Null,
            intent: Nullable::Null,
            arguments: Opaque::from_bytes(Vec::new()),
            budget: if spec.has(Annotation::TaskStarting) {
                Optional::Present(budget())
            } else {
                Optional::Absent
            },
            output_policy: Optional::Absent,
            trace: Optional::Absent,
            page: Optional::Absent,
        };
        self.daemon.dispatch(&OperationRequest {
            envelope,
            arguments,
        })
    }

    /// Mint the held key for `kinds`; a key that signs bundles also signs the acceptances
    /// its bundles carry.
    fn mint(&mut self, kinds: &[SignedArtifactKind]) -> (SignerHandle, SignerIdentity) {
        let mut kinds = kinds.to_vec();
        if kinds.contains(&SignedArtifactKind::IntentBundle)
            && !kinds.contains(&SignedArtifactKind::IntentAcceptance)
        {
            kinds.push(SignedArtifactKind::IntentAcceptance);
        }
        let outcome = self.call(
            STEWARD,
            Arguments::SigningMint(SigningMintRequest { kinds }),
        );
        let Payload::SigningMint(minted) = outcome.payload else {
            panic!("mint failed: {:?}", outcome.error_code());
        };
        let identity = SignerIdentity::from_public_key(&minted.public_key).expect("a key");
        assert_eq!(identity.handle().as_str(), minted.signer.as_str());
        (minted.signer, identity)
    }

    fn records(&self) -> usize {
        self.daemon.state().signing().registry().audit_log().len()
    }

    /// Enter `contract` as proposed and accept it through the real `intent.accept`: at 3.8
    /// a daemon whose key may sign acceptances records the RFC 0037 A1 chain.
    fn put_accepted(
        &mut self,
        contract: &IntentContract,
        supersedes: Option<IntentHandle>,
    ) -> IntentHandle {
        let handle = intent_handle(contract);
        self.daemon.state_mut().put_intent(
            handle.clone(),
            IntentRecord {
                contract: contract.clone(),
                status: RegistryStatus::Proposed,
                supersedes,
                superseded_by: None,
                acceptance: None,
            },
        );
        let accepted = self.accept(&handle, None, CALLER_TEXT);
        assert_eq!(accepted.error_code(), None, "a local acceptance");
        handle
    }

    fn export(&mut self, intents: Vec<IntentHandle>) -> (IntentBundleHandle, Vec<u8>) {
        let outcome = self.call(
            STEWARD,
            Arguments::IntentExportBundle(IntentExportBundleRequest { intents }),
        );
        let Payload::IntentExportBundle(exported) = outcome.payload else {
            panic!("export failed: {:?}", outcome.envelope.error);
        };
        (exported.bundle, exported.content)
    }

    fn import(&mut self, content: Vec<u8>) -> OperationOutcome {
        self.call(
            STEWARD,
            Arguments::IntentImportBundle(IntentImportBundleRequest { content }),
        )
    }

    /// `intent.accept`. With a bundle and [`FROM_BUNDLE`], present the acceptance the held
    /// bundle's record carries for `proposal` — what a CI job reads from the bundle.
    fn accept(
        &mut self,
        proposal: &IntentHandle,
        bundle: Option<&IntentBundleHandle>,
        signature: &str,
    ) -> OperationOutcome {
        let presented = match bundle {
            Some(bundle) if signature == FROM_BUNDLE => self
                .daemon
                .state()
                .signing()
                .bundle(bundle)
                .and_then(|held| held.body().contract(proposal))
                .and_then(|entry| record_signature(&entry.record))
                .unwrap_or_else(|| "not-held".to_owned()),
            _ => signature.to_owned(),
        };
        self.call(
            STEWARD,
            Arguments::IntentAccept(IntentAcceptRequest {
                proposal: proposal.clone(),
                acceptance: acceptance(STEWARD.0, &presented, ACCEPTED_AT),
                bundle: bundle.map_or(Optional::Absent, |bundle| Optional::Present(bundle.clone())),
            }),
        )
    }

    fn verify(
        &mut self,
        kind: SignedArtifactKind,
        artifact: &[u8],
        signature: Option<Vec<u8>>,
    ) -> SignatureOutcome {
        let outcome = self.call(
            READER,
            Arguments::SigningVerify(SigningVerifyRequest {
                kind,
                artifact: artifact.to_vec(),
                signature: signature.map_or(Optional::Absent, Optional::Present),
            }),
        );
        let Payload::SigningVerify(verified) = outcome.payload else {
            panic!("verify failed: {:?}", outcome.error_code());
        };
        verified.outcome
    }
}

fn budget() -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Absent,
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// Present the acceptance the bundle's record carries (see [`World::accept`]).
const FROM_BUNDLE: &str = "";

/// The `signature` text a caller presents on a local acceptance. A daemon whose key may
/// sign acceptances replaces it with its own A1 signature; nothing verifies it otherwise.
const CALLER_TEXT: &str = "caller-attestation";

/// The acceptance `signature` a registry record's JSON carries.
fn record_signature(record: &[u8]) -> Option<String> {
    let Ok(ContractJson::Object(fields)) = ContractJson::parse(record) else {
        return None;
    };
    let Some(ContractJson::Object(acceptance)) = fields.get("acceptance") else {
        return None;
    };
    match acceptance.get("signature") {
        Some(ContractJson::String(signature)) => Some(signature.clone()),
        _ => None,
    }
}

/// The kinds a pinned bundle exporter needs: its bundles, and the acceptances in them.
const BUNDLE_KINDS: [Kind; 2] = [Kind::IntentBundle, Kind::IntentAcceptance];

fn acceptance(by: &str, signature: &str, at: &str) -> Opaque {
    let mut fields = BTreeMap::new();
    for (key, value) in [
        ("accepted_by", by),
        ("capability", "revise-intent"),
        ("signature", signature),
        ("timestamp", at),
    ] {
        fields.insert(key.to_owned(), ContractJson::String(value.to_owned()));
    }
    Opaque::from_bytes(ContractJson::Object(fields).to_canonical_bytes())
}

/// The Die Hard contract, stamped with the `in_` its preimage names (W9), and optionally
/// renamed so two fixtures are two identities.
fn contract(variant: Option<u32>) -> IntentContract {
    let text = DIE_HARD_CONTRACT.trim_end();
    let text = match variant {
        // The `faults` bound is in the preimage, so each variant is its own identity.
        Some(faults) => text.replace("\"faults\":0", &format!("\"faults\":{faults}")),
        None => text.to_owned(),
    };
    let unstamped = IntentContract::decode(text.as_bytes()).expect("the fixture decodes");
    let handle = intent_handle(&unstamped);
    let stamped = text.replacen(
        "\"intent_id\":\"in_die_hard_v1\"",
        &format!("\"intent_id\":\"{}\"", handle.as_str()),
        1,
    );
    let contract = IntentContract::decode(stamped.as_bytes()).expect("the stamped fixture decodes");
    assert_eq!(
        intent_handle(&contract),
        handle,
        "intent_id is outside the preimage"
    );
    contract
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    continuumd::daemon::identity::intent_to_wire(&stored).expect("an in_ handle")
}

fn signer_handle(identity: &SignerIdentity) -> SignerHandle {
    SignerHandle::new(identity.handle().as_str()).expect("a signer name")
}

fn import_outcome(outcome: &OperationOutcome) -> (SignatureOutcome, u32, usize) {
    let Payload::IntentImportBundle(imported) = &outcome.payload else {
        panic!("import failed: {:?}", outcome.envelope.error);
    };
    (imported.outcome, imported.adopted, imported.imported.len())
}

/// An exporter that minted its key and holds one accepted contract, and an importer that
/// pins the exporter's signer (plan §18.6's organizational deployment).
fn exporter_and_importer() -> (World, World, IntentHandle, SignerIdentity) {
    let mut exporter = with_entropy(10);
    let (_, identity) = exporter.mint(&[
        SignedArtifactKind::IntentBundle,
        SignedArtifactKind::Receipt,
    ]);
    let proposal = exporter.put_accepted(&contract(None), None);
    let importer = world(Setup {
        entropy: Some(100),
        allowed: Some(AllowedSigners::new().allow(identity.clone(), BUNDLE_KINDS)),
        ..Setup::default()
    });
    (exporter, importer, proposal, identity)
}

// --- authority -------------------------------------------------------------------------------

fn every_signing_wire_request() -> Vec<Arguments> {
    let signer = SignerHandle::new(&format!("signer_{}", "0".repeat(64))).expect("name");
    vec![
        Arguments::SigningMint(SigningMintRequest {
            kinds: vec![SignedArtifactKind::Receipt],
        }),
        Arguments::SigningRotate(SigningRotateRequest {
            signer: signer.clone(),
        }),
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer,
            reason: RevocationReason::Compromised,
        }),
        Arguments::SigningRegistry(SigningRegistryRequest {}),
        Arguments::SigningVerify(SigningVerifyRequest {
            kind: SignedArtifactKind::Receipt,
            artifact: b"x".to_vec(),
            signature: Optional::Absent,
        }),
        Arguments::SigningSignPack(SigningSignPackRequest {
            pack: b"pack".to_vec(),
        }),
        Arguments::IntentExportBundle(IntentExportBundleRequest {
            intents: vec![IntentHandle::new("in_somewhere").expect("an in_")],
        }),
        Arguments::IntentImportBundle(IntentImportBundleRequest {
            content: b"not a bundle".to_vec(),
        }),
    ]
}

#[test]
fn every_signing_wire_operation_is_refused_to_a_scoped_grant() {
    // The scoped grant holds every privilege and the highest level, and its only difference
    // from the steward is one `intents` entry. The same request is denied at admission for
    // it and reaches the handler for the steward.
    let mut world = with_entropy(1);
    for arguments in every_signing_wire_request() {
        let operation = arguments.operation();
        let before = world.daemon.state().admissions().len();
        let denied = world.call(SCOPED, arguments.clone());
        assert_eq!(
            denied.error_code(),
            Some(ErrorCode::CapabilityDenied),
            "{operation}"
        );
        let record = &world.daemon.state().admissions()[before];
        assert!(!record.admitted, "{operation}: refused at admission");
        let _ = world.call(STEWARD, arguments);
        let record = world.daemon.state().admissions().last().expect("recorded");
        assert!(
            record.admitted,
            "{operation}: the unscoped steward is admitted"
        );
    }
    assert_eq!(
        world.records(),
        1,
        "only the steward's mint reached the registry"
    );
}

#[test]
fn the_writes_are_refused_without_the_privilege() {
    let mut world = with_entropy(1);
    for arguments in every_signing_wire_request() {
        let operation = arguments.operation();
        let privileged = registry::operation(operation)
            .expect("declared")
            .has(Annotation::Privileged);
        let outcome = world.call(REVISER, arguments);
        if privileged {
            assert_eq!(
                outcome.error_code(),
                Some(ErrorCode::CapabilityDenied),
                "{operation}"
            );
        }
    }
    assert_eq!(
        world.records(),
        0,
        "nothing was minted without the privilege"
    );
    // And the two `read` operations are open to an unscoped reader.
    let registry = world.call(
        READER,
        Arguments::SigningRegistry(SigningRegistryRequest {}),
    );
    assert_eq!(registry.error_code(), None);
}

#[test]
fn the_signing_wire_is_refused_below_protocol_3_8() {
    // Refused before admission and before the idempotency ledger: the answer is the same for
    // the privileged steward and for a scoped grant admission would deny, and no admission
    // decision is recorded.
    let mut world = world(Setup {
        entropy: Some(1),
        version: Some(ProtocolVersion::new(3, 7)),
        ..Setup::default()
    });
    for principal in [STEWARD, SCOPED, READER] {
        for arguments in every_signing_wire_request() {
            let operation = arguments.operation();
            let before = world.daemon.state().admissions().len();
            let outcome = world.call(principal, arguments);
            assert_eq!(
                outcome.error_code(),
                Some(ErrorCode::MalformedRequest),
                "{operation} is not defined at 3.7"
            );
            assert_eq!(
                world.daemon.state().admissions().len(),
                before,
                "{operation}: refused before admission"
            );
        }
    }
    assert_eq!(world.records(), 0);
}

// --- lifecycle ---------------------------------------------------------------------------

#[test]
fn mint_rotate_revoke_move_the_registry_forward_only() {
    let mut world = with_entropy(1);
    let (first, first_identity) = world.mint(&[SignedArtifactKind::Receipt]);
    assert_eq!(world.records(), 1);
    assert_eq!(world.daemon.state().signing().held(), Some(&first_identity));

    // A second mint while the key is active is refused, and records nothing.
    let again = world.call(
        STEWARD,
        Arguments::SigningMint(SigningMintRequest {
            kinds: vec![SignedArtifactKind::Receipt],
        }),
    );
    assert_eq!(again.error_code(), Some(ErrorCode::PolicyGateFailed));
    assert_eq!(world.records(), 1);

    let rotated = world.call(
        STEWARD,
        Arguments::SigningRotate(SigningRotateRequest {
            signer: first.clone(),
        }),
    );
    let Payload::SigningRotate(rotation) = rotated.payload else {
        panic!("rotate failed: {:?}", rotated.error_code());
    };
    assert_eq!(rotation.retired, first);
    assert_ne!(rotation.successor, first);
    assert_eq!(world.records(), 3, "a successor mint and the rotation");

    // The retired key cannot be rotated again, and the successor inherits its kinds.
    let stale = world.call(
        STEWARD,
        Arguments::SigningRotate(SigningRotateRequest {
            signer: first.clone(),
        }),
    );
    assert_eq!(stale.error_code(), Some(ErrorCode::PolicyGateFailed));
    let successor = SignerIdentity::from_public_key(&rotation.public_key).expect("a key");
    assert!(
        world
            .daemon
            .state()
            .signing()
            .allowed()
            .permits(&successor, Kind::Receipt)
    );

    // Revoke the retired key, then fail to revoke it again: revocation is final.
    let revoked = world.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: first.clone(),
            reason: RevocationReason::Compromised,
        }),
    );
    assert_eq!(revoked.error_code(), None);
    let twice = world.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: first,
            reason: RevocationReason::KeyLost,
        }),
    );
    assert_eq!(twice.error_code(), Some(ErrorCode::PolicyGateFailed));

    // `key-lost` on the held key supersedes it with a linked successor.
    let lost = world.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: rotation.successor.clone(),
            reason: RevocationReason::KeyLost,
        }),
    );
    let Payload::SigningRevoke(superseded) = lost.payload else {
        panic!("supersede failed: {:?}", lost.error_code());
    };
    assert!(matches!(superseded.successor, Nullable::Value(_)));
    assert_eq!(world.records(), 7, "revoke, then mint + revoke + supersede");

    // The registry read reports every record in order, and the log only ever grew.
    let read = world.call(
        READER,
        Arguments::SigningRegistry(SigningRegistryRequest {}),
    );
    let Payload::SigningRegistry(registry) = read.payload else {
        panic!("registry read failed");
    };
    assert_eq!(registry.log.len(), 7);
    assert_eq!(registry.signers.len(), 3);
    let replayed = SigningRegistry::replay(
        &continuum_value::value::Value::seq(
            registry
                .log
                .iter()
                .map(|bytes| continuum_value::value::Value::decode(bytes).expect("canonical")),
        )
        .expect("a sequence"),
    )
    .expect("the wire log replays");
    assert_eq!(&replayed, world.daemon.state().signing().registry());
    assert_eq!(registry.registry_head, replayed.head().digest().to_vec());
    let _ = first_identity;
}

#[test]
fn a_replayed_rotation_does_not_rotate_twice() {
    let mut world = with_entropy(1);
    let (first, _) = world.mint(&[SignedArtifactKind::Receipt]);
    let request = Arguments::SigningRotate(SigningRotateRequest { signer: first });
    let once = world.call_keyed(STEWARD, request.clone(), "rotate-once");
    let records = world.records();
    let held = world.daemon.state().signing().held().cloned();
    let again = world.call_keyed(STEWARD, request, "rotate-once");
    assert_eq!(
        again.payload, once.payload,
        "the replay returns the recorded outcome"
    );
    assert_eq!(world.records(), records, "and records nothing");
    assert_eq!(world.daemon.state().signing().held().cloned(), held);
}

#[test]
fn rotation_needs_the_held_key() {
    // A signer the registry knows but the daemon does not hold cannot be rotated: rotation
    // needs possession. A name the registry does not hold is refused the same way.
    let (mut exporter, mut importer, _, identity) = exporter_and_importer();
    let proposal = exporter.put_accepted(&contract(Some(1)), None);
    let (_, content) = exporter.export(vec![proposal]);
    let _ = importer.import(content);
    assert!(
        importer
            .daemon
            .state()
            .signing()
            .registry()
            .standing(&identity)
            .is_some()
    );
    let foreign = importer.call(
        STEWARD,
        Arguments::SigningRotate(SigningRotateRequest {
            signer: signer_handle(&identity),
        }),
    );
    assert_eq!(foreign.error_code(), Some(ErrorCode::PolicyGateFailed));
    let unknown = importer.call(
        STEWARD,
        Arguments::SigningRotate(SigningRotateRequest {
            signer: SignerHandle::new(&format!("signer_{}", "1".repeat(64))).expect("name"),
        }),
    );
    assert_eq!(unknown.error_code(), Some(ErrorCode::PolicyGateFailed));
}

#[test]
fn mint_without_an_entropy_capability_records_nothing() {
    let mut world = world(Setup::default());
    let outcome = world.call(
        STEWARD,
        Arguments::SigningMint(SigningMintRequest {
            kinds: vec![SignedArtifactKind::Receipt],
        }),
    );
    assert_eq!(
        outcome.error_code(),
        Some(ErrorCode::UnsupportedSemanticFeature)
    );
    assert_eq!(world.records(), 0);
    let empty = world.call(
        STEWARD,
        Arguments::SigningMint(SigningMintRequest { kinds: Vec::new() }),
    );
    assert_eq!(empty.error_code(), Some(ErrorCode::MalformedRequest));
}

#[test]
fn no_answer_fault_or_debug_output_carries_key_material() {
    // Seeds are `[n; 32]`. Every seed this world drew is looked for, as raw bytes in every
    // encoded payload and as hex in every `Debug` rendering, including the daemon's own.
    let mut world = with_entropy(40);
    let mut outcomes = Vec::new();
    let (first, _) = world.mint(&[SignedArtifactKind::Receipt, SignedArtifactKind::DomainPack]);
    outcomes.push(world.call(
        STEWARD,
        Arguments::SigningRotate(SigningRotateRequest {
            signer: first.clone(),
        }),
    ));
    outcomes.push(world.call(
        STEWARD,
        Arguments::SigningRotate(SigningRotateRequest { signer: first }),
    ));
    outcomes.push(world.call(
        STEWARD,
        Arguments::SigningSignPack(SigningSignPackRequest {
            pack: b"p".to_vec(),
        }),
    ));
    outcomes.push(world.call(
        READER,
        Arguments::SigningRegistry(SigningRegistryRequest {}),
    ));
    outcomes.push(world.call(
        STEWARD,
        Arguments::SigningMint(SigningMintRequest {
            kinds: vec![SignedArtifactKind::Receipt],
        }),
    ));
    let seeds: Vec<[u8; SEED_LEN]> = (41..=43).map(|n| [n; SEED_LEN]).collect();
    let debug = format!("{:?} {:?}", world.daemon, outcomes);
    for seed in &seeds {
        let hex: String = seed.iter().map(|byte| format!("{byte:02x}")).collect();
        assert!(!debug.contains(&hex), "a seed reached Debug output");
        let list = format!("{:?}", &seed[..]);
        assert!(
            !debug.contains(&list[1..list.len() - 1]),
            "a seed reached Debug output"
        );
        for outcome in &outcomes {
            if let Ok(Some(payload)) =
                continuumd::codec::operations::encode_payload(&outcome.payload)
            {
                assert!(
                    !payload
                        .as_bytes()
                        .windows(SEED_LEN)
                        .any(|window| window == seed),
                    "a seed reached an encoded payload"
                );
            }
            if let Some(error) = outcome.envelope.error.value() {
                assert!(!error.detail.contains(&hex));
            }
        }
    }
}

// --- verification --------------------------------------------------------------------------

#[test]
fn signing_verify_is_total_and_typed() {
    let mut world = with_entropy(1);
    let (_, identity) = world.mint(&[SignedArtifactKind::DomainPack]);
    let pack = b"{\"schema_id\":\"https://continuum.dev/schema/domain-pack.json\"}".to_vec();
    let signed = world.call(
        STEWARD,
        Arguments::SigningSignPack(SigningSignPackRequest { pack: pack.clone() }),
    );
    let Payload::SigningSignPack(signature) = signed.payload else {
        panic!("sign_pack failed");
    };
    let sig = signature.signature.clone();
    assert_eq!(
        world.verify(SignedArtifactKind::DomainPack, &pack, Some(sig.clone())),
        SignatureOutcome::Verified
    );
    assert_eq!(
        world.verify(SignedArtifactKind::DomainPack, &pack, None),
        SignatureOutcome::Unsigned
    );
    assert_eq!(
        world.verify(
            SignedArtifactKind::DomainPack,
            b"tampered",
            Some(sig.clone())
        ),
        SignatureOutcome::SignatureMismatch
    );
    assert_eq!(
        world.verify(SignedArtifactKind::Receipt, &pack, Some(sig.clone())),
        SignatureOutcome::KindMismatch
    );
    assert_eq!(
        world.verify(
            SignedArtifactKind::DomainPack,
            &pack,
            Some(sig[..sig.len() - 1].to_vec())
        ),
        SignatureOutcome::Malformed
    );
    assert_eq!(
        world.verify(SignedArtifactKind::DomainPack, &pack, Some(vec![0; 4096])),
        SignatureOutcome::Malformed
    );

    // A signer this registry has never heard of is never verified.
    let mut stranger = SigningRegistry::new();
    let key = stranger
        .mint(
            &SigningActor::new("human:x").expect("actor"),
            &mut Seeds(200),
        )
        .expect("mint");
    let foreign = stranger
        .sign(&key, Kind::DomainPack, &signed_bytes_identity(&pack))
        .expect("sign");
    assert_eq!(
        world.verify(
            SignedArtifactKind::DomainPack,
            &pack,
            Some(foreign.encode())
        ),
        SignatureOutcome::StandingUnknown
    );

    // Signed as a receipt kind the key is not allowed for: authentic, not allowed.
    let receipt_signature = world.daemon.state().signing().registry().clone();
    let _ = receipt_signature;
    // Revoked: the same pack signature now downgrades.
    let _ = world.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: signer_handle(&identity),
            reason: RevocationReason::Compromised,
        }),
    );
    assert_eq!(
        world.verify(SignedArtifactKind::DomainPack, &pack, Some(sig)),
        SignatureOutcome::SignerRevoked
    );

    // An artifact over the bound is refused before it is hashed.
    let over = world.call(
        READER,
        Arguments::SigningVerify(SigningVerifyRequest {
            kind: SignedArtifactKind::DomainPack,
            artifact: vec![0; (1 << 20) + 1],
            signature: Optional::Absent,
        }),
    );
    assert_eq!(over.error_code(), Some(ErrorCode::MalformedRequest));
}

#[test]
fn a_signer_allowed_for_one_kind_is_not_verified_for_another() {
    // The exporter's key signs bundles and packs; the importer pins it for bundles only. A
    // pack it signed is authentic and known to the importer, and still not verified there.
    let mut exporter = with_entropy(10);
    let (_, identity) = exporter.mint(&[
        SignedArtifactKind::IntentBundle,
        SignedArtifactKind::DomainPack,
    ]);
    let proposal = exporter.put_accepted(&contract(None), None);
    let (_, content) = exporter.export(vec![proposal]);
    let signed = exporter.call(
        STEWARD,
        Arguments::SigningSignPack(SigningSignPackRequest {
            pack: b"p".to_vec(),
        }),
    );
    let Payload::SigningSignPack(signature) = signed.payload else {
        panic!("sign_pack failed");
    };
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(AllowedSigners::new().allow(identity, BUNDLE_KINDS)),
        ..Setup::default()
    });
    let _ = importer.import(content);
    assert_eq!(
        importer.verify(
            SignedArtifactKind::DomainPack,
            b"p",
            Some(signature.signature)
        ),
        SignatureOutcome::SignerNotAllowed
    );
}

#[test]
fn a_revoked_held_key_refuses_to_sign_anything() {
    let mut world = with_entropy(1);
    let (held, _) = world.mint(&[
        SignedArtifactKind::Receipt,
        SignedArtifactKind::IntentBundle,
    ]);
    let _ = world.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: held,
            reason: RevocationReason::Compromised,
        }),
    );
    let pack = world.call(
        STEWARD,
        Arguments::SigningSignPack(SigningSignPackRequest {
            pack: b"p".to_vec(),
        }),
    );
    assert_eq!(pack.error_code(), Some(ErrorCode::PolicyGateFailed));
    // Nor does it sign an acceptance: the local acceptance is refused, not recorded
    // unsigned (a_signed_local_acceptance_names_only_the_admitted_principal_at_the_daemons_time).
    let proposal = intent_handle(&contract(None));
    world.daemon.state_mut().put_intent(
        proposal.clone(),
        IntentRecord {
            contract: contract(None),
            status: RegistryStatus::Accepted,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );
    let export = world.call(
        STEWARD,
        Arguments::IntentExportBundle(IntentExportBundleRequest {
            intents: vec![proposal],
        }),
    );
    assert_eq!(export.error_code(), Some(ErrorCode::PolicyGateFailed));
    // A new mint is admitted once the held key is no longer active.
    let (_, fresh) = world.mint(&[SignedArtifactKind::Receipt]);
    assert_eq!(world.daemon.state().signing().held(), Some(&fresh));
}

// --- receipts and packs -----------------------------------------------------------------------

#[test]
fn a_receipt_signature_travels_on_evidence_get_and_verifies_over_the_wire() {
    let mut registry = SigningRegistry::new();
    let key = registry
        .mint(
            &SigningActor::new("human:solo-dev").expect("actor"),
            &mut Seeds(7),
        )
        .expect("mint");
    let identity = key.identity().clone();
    let mut world = world(Setup {
        signer: Some(ReceiptSigner::new(registry, key).expect("installable")),
        ..Setup::default()
    });
    let receipt_content = b"{\"receipt\":\"checked\"}\n".to_vec();
    let stage = |daemon: &mut Daemon, path: &str, content: &[u8]| {
        daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(path).expect("path"),
                content.to_vec(),
            )
            .expect("staged")
    };
    let trace = stage(&mut world.daemon, "traces/t.jsonl", b"{\"events\":[]}\n");
    let receipt = stage(&mut world.daemon, "receipts/r.json", &receipt_content);
    let ingested = world.call(
        PRODUCER,
        Arguments::ObserveIngest(ObserveIngestRequest {
            trace,
            instrumentation_profile: "otel-1.0/sampled".to_owned(),
        }),
    );
    let Payload::ObserveIngest(ingest) = ingested.payload else {
        panic!("ingest failed: {:?}", ingested.envelope.error);
    };
    let linked = world.call(
        CHECKER,
        Arguments::EvidenceLink(EvidenceLinkRequest {
            subject: ingest.evidence[0].clone(),
            receipt,
            checker_profile: "kernel-core/1".to_owned(),
        }),
    );
    let Payload::EvidenceLink(link) = linked.payload else {
        panic!("link failed: {:?}", linked.envelope.error);
    };
    let got = world.call(
        READER,
        Arguments::EvidenceGet(EvidenceGetRequest {
            evidence: link.receipt.clone(),
            inline: Optional::Absent,
        }),
    );
    let Payload::EvidenceGet(node) = got.payload else {
        panic!("evidence.get failed");
    };
    let Optional::Present(signature) = node.signature else {
        panic!("a signed receipt carries its signature at 3.8");
    };
    assert_eq!(
        ArtifactSignature::decode(&signature)
            .expect("canonical")
            .signer(),
        &identity
    );
    assert_eq!(
        world.verify(
            SignedArtifactKind::Receipt,
            &receipt_content,
            Some(signature.clone())
        ),
        SignatureOutcome::Verified
    );
    assert_eq!(
        world.verify(SignedArtifactKind::Receipt, b"other", Some(signature)),
        SignatureOutcome::SignatureMismatch
    );

    // An unsigned node carries none.
    let subject = world.call(
        READER,
        Arguments::EvidenceGet(EvidenceGetRequest {
            evidence: ingest.evidence[0].clone(),
            inline: Optional::Absent,
        }),
    );
    let Payload::EvidenceGet(subject) = subject.payload else {
        panic!("evidence.get failed");
    };
    assert_eq!(subject.signature, Optional::Absent);
}

#[test]
fn a_receipt_signature_is_kept_off_a_connection_below_3_8() {
    let mut registry = SigningRegistry::new();
    let key = registry
        .mint(
            &SigningActor::new("human:solo-dev").expect("actor"),
            &mut Seeds(7),
        )
        .expect("mint");
    let mut world = world(Setup {
        signer: Some(ReceiptSigner::new(registry, key).expect("installable")),
        version: Some(ProtocolVersion::new(3, 7)),
        ..Setup::default()
    });
    let trace = world
        .daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("t.jsonl").expect("path"),
            b"{}\n".to_vec(),
        )
        .expect("staged");
    let receipt = world
        .daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("r.json").expect("path"),
            b"{\"r\":1}\n".to_vec(),
        )
        .expect("staged");
    let ingested = world.call(
        PRODUCER,
        Arguments::ObserveIngest(ObserveIngestRequest {
            trace,
            instrumentation_profile: "otel-1.0/sampled".to_owned(),
        }),
    );
    let Payload::ObserveIngest(ingest) = ingested.payload else {
        panic!("ingest")
    };
    let linked = world.call(
        CHECKER,
        Arguments::EvidenceLink(EvidenceLinkRequest {
            subject: ingest.evidence[0].clone(),
            receipt,
            checker_profile: "k/1".to_owned(),
        }),
    );
    let Payload::EvidenceLink(link) = linked.payload else {
        panic!("link")
    };
    assert!(
        world
            .daemon
            .state()
            .receipt_signature(&link.receipt)
            .is_some(),
        "it was signed"
    );
    let got = world.call(
        READER,
        Arguments::EvidenceGet(EvidenceGetRequest {
            evidence: link.receipt,
            inline: Optional::Absent,
        }),
    );
    let Payload::EvidenceGet(node) = got.payload else {
        panic!("get")
    };
    assert_eq!(
        node.signature,
        Optional::Absent,
        "3.7 does not define the field"
    );
}

#[test]
fn a_domain_pack_signed_over_the_wire_verifies_and_a_tampered_one_does_not() {
    let mut world = with_entropy(3);
    let (signer, _) = world.mint(&[SignedArtifactKind::DomainPack]);
    let pack = b"a domain pack manifest".to_vec();
    let signed = world.call(
        STEWARD,
        Arguments::SigningSignPack(SigningSignPackRequest { pack: pack.clone() }),
    );
    let Payload::SigningSignPack(result) = signed.payload else {
        panic!("sign_pack failed");
    };
    assert_eq!(result.signer, signer);
    assert_eq!(
        world.verify(
            SignedArtifactKind::DomainPack,
            &pack,
            Some(result.signature.clone())
        ),
        SignatureOutcome::Verified
    );
    let mut tampered = pack;
    tampered[0] ^= 1;
    assert_eq!(
        world.verify(
            SignedArtifactKind::DomainPack,
            &tampered,
            Some(result.signature)
        ),
        SignatureOutcome::SignatureMismatch
    );
}

// --- bundles ----------------------------------------------------------------------------------

#[test]
fn the_daemons_bundle_signature_matches_the_library_signing_the_same_body() {
    // Differential: the daemon's export and the library signing the exported body with the
    // same seed produce the same record, byte for byte.
    let (mut exporter, _, proposal, _) = exporter_and_importer();
    let (_, content) = exporter.export(vec![proposal]);
    let signed = decode_signed(&content).expect("the export reads back");
    let mut library = SigningRegistry::new();
    let key = library
        .mint(
            &SigningActor::new(STEWARD.0).expect("actor"),
            &mut Seeds(10),
        )
        .expect("mint");
    let oracle = library
        .sign(
            &key,
            Kind::IntentBundle,
            &signed_bytes_identity(signed.body_bytes()),
        )
        .expect("sign");
    assert_eq!(oracle.encode(), signed.signature());
    assert_eq!(
        signed.body().encode(),
        signed.body_bytes(),
        "the body re-encodes to itself"
    );
}

#[test]
fn an_exported_bundle_imports_and_its_chain_accepts_the_proposal() {
    let (mut exporter, mut importer, proposal, identity) = exporter_and_importer();
    let (bundle, content) = exporter.export(vec![proposal.clone()]);

    let imported = importer.import(content);
    let (outcome, adopted, entered) = import_outcome(&imported);
    assert_eq!(outcome, SignatureOutcome::Verified);
    assert_eq!(
        adopted, 1,
        "the pinned signer's mint is the one fact the log carries"
    );
    assert_eq!(entered, 1);
    let record = importer.daemon.state().intent(&proposal).expect("imported");
    assert_eq!(
        record.status,
        RegistryStatus::Proposed,
        "import never raises status (I2)"
    );
    assert!(
        importer
            .daemon
            .state()
            .signing()
            .registry()
            .standing(&identity)
            .is_some()
    );

    let accepted = importer.accept(&proposal, Some(&bundle), FROM_BUNDLE);
    assert_eq!(accepted.error_code(), None, "{:?}", accepted.envelope.error);
    assert_eq!(
        importer
            .daemon
            .state()
            .intent(&proposal)
            .expect("held")
            .status,
        RegistryStatus::Accepted
    );
}

#[test]
fn an_importer_with_its_own_key_still_adopts_and_accepts() {
    // Standing is adopted as facts, not as a verbatim log, so an importer that minted its
    // own key first is not forked away from its exporter.
    let (mut exporter, mut importer, proposal, identity) = exporter_and_importer();
    let (bundle, content) = exporter.export(vec![proposal.clone()]);
    let (_, own) = importer.mint(&[SignedArtifactKind::Receipt]);
    let (outcome, adopted, _) = import_outcome(&importer.import(content));
    assert_eq!(outcome, SignatureOutcome::Verified);
    assert_eq!(adopted, 1);
    assert!(
        importer
            .daemon
            .state()
            .signing()
            .registry()
            .standing(&identity)
            .is_some()
    );
    assert_eq!(importer.daemon.state().signing().held(), Some(&own));
    assert_eq!(
        importer
            .accept(&proposal, Some(&bundle), FROM_BUNDLE)
            .error_code(),
        None
    );
}

#[test]
fn accept_fails_closed_on_every_broken_chain() {
    let chain_invalid = Some(ErrorCode::AcceptanceChainInvalid);

    // An absent bundle.
    let (mut exporter, mut importer, proposal, identity) = exporter_and_importer();
    let (bundle, content) = exporter.export(vec![proposal.clone()]);
    let _ = importer.import(content.clone());
    let absent = IntentBundleHandle::new("inb_absent").expect("an inb_");
    assert_eq!(
        importer
            .accept(&proposal, Some(&absent), FROM_BUNDLE)
            .error_code(),
        chain_invalid
    );

    // An acceptance the signed record does not carry.
    let other = format!("ed25519:{}", "cd".repeat(64));
    assert_eq!(
        importer
            .accept(&proposal, Some(&bundle), &other)
            .error_code(),
        chain_invalid
    );

    // A lineage the signed record does not carry (A2).
    importer
        .daemon
        .state_mut()
        .intent_mut(&proposal)
        .expect("held")
        .supersedes = Some(IntentHandle::new("in_elsewhere").expect("an in_"));
    assert_eq!(
        importer
            .accept(&proposal, Some(&bundle), FROM_BUNDLE)
            .error_code(),
        chain_invalid
    );
    importer
        .daemon
        .state_mut()
        .intent_mut(&proposal)
        .expect("held")
        .supersedes = None;

    // A proposal the bundle does not export.
    let stray = importer.put_accepted(&contract(Some(2)), None);
    importer
        .daemon
        .state_mut()
        .intent_mut(&stray)
        .expect("held")
        .status = RegistryStatus::Proposed;
    assert_eq!(
        importer
            .accept(&stray, Some(&bundle), FROM_BUNDLE)
            .error_code(),
        chain_invalid
    );

    // The signer revoked locally after the import.
    let revoked = importer.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: signer_handle(&identity),
            reason: RevocationReason::Compromised,
        }),
    );
    assert_eq!(revoked.error_code(), None);
    assert_eq!(
        importer
            .accept(&proposal, Some(&bundle), FROM_BUNDLE)
            .error_code(),
        chain_invalid
    );
    assert_eq!(
        importer
            .daemon
            .state()
            .intent(&proposal)
            .expect("held")
            .status,
        RegistryStatus::Proposed,
        "nothing was accepted"
    );

    // A signer local policy does not pin: its standing is never learned, the bundle does
    // not verify, and nothing is entered, adopted, or held.
    let mut unpinned = world(Setup {
        entropy: Some(60),
        ..Setup::default()
    });
    let (outcome, adopted, entered) = import_outcome(&unpinned.import(content));
    assert_eq!(outcome, SignatureOutcome::StandingUnknown);
    assert_eq!(
        (adopted, entered),
        (0, 0),
        "an unverified bundle changes nothing"
    );
    assert_eq!(unpinned.records(), 0);
    assert!(unpinned.daemon.state().intent(&proposal).is_none());
    assert!(
        unpinned.daemon.state().signing().bundle(&bundle).is_none(),
        "and is not held"
    );
    assert_eq!(
        unpinned
            .accept(&proposal, Some(&bundle), FROM_BUNDLE)
            .error_code(),
        chain_invalid
    );
}

#[test]
fn an_imported_proposal_is_accepted_only_through_a_verified_bundle() {
    let (mut exporter, mut importer, proposal, _) = exporter_and_importer();
    let (_, content) = exporter.export(vec![proposal.clone()]);
    let _ = importer.import(content);
    // A local acceptance, and a lock that would mint an accepted successor, are both
    // refused for a proposal an import entered.
    assert_eq!(
        importer.accept(&proposal, None, CALLER_TEXT).error_code(),
        Some(ErrorCode::AcceptanceChainInvalid)
    );
    let lock = importer.call(
        STEWARD,
        Arguments::IntentLock(
            continuumd::protocol::operations::intent::IntentLockRequest {
                intent: proposal.clone(),
                policy: BTreeMap::new(),
            },
        ),
    );
    assert_eq!(lock.error_code(), Some(ErrorCode::IntentMutationDenied));
    assert_eq!(
        importer
            .daemon
            .state()
            .intent(&proposal)
            .expect("held")
            .status,
        RegistryStatus::Proposed
    );
}

#[test]
fn an_imported_revision_may_not_move_a_field_its_predecessor_protects() {
    // Q is accepted; P supersedes it and changes `bounds`, whose verb is `no-decrease`. The
    // classifier that would decide the direction has not shipped, so the import refuses P
    // and enters nothing — not even Q.
    let mut exporter = with_entropy(10);
    let (_, identity) = exporter.mint(&[SignedArtifactKind::IntentBundle]);
    let q = exporter.put_accepted(&contract(None), None);
    let p = exporter.put_accepted(&contract(Some(1)), Some(q.clone()));
    let (_, content) = exporter.export(vec![q.clone(), p.clone()]);
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(AllowedSigners::new().allow(identity, BUNDLE_KINDS)),
        ..Setup::default()
    });
    let refused = importer.import(content);
    assert_eq!(refused.error_code(), Some(ErrorCode::IntentMutationDenied));
    assert!(importer.daemon.state().intent(&q).is_none());
    assert!(importer.daemon.state().intent(&p).is_none());
    assert_eq!(importer.records(), 0, "nothing was adopted either");
}

#[test]
fn an_acceptance_needs_its_predecessor_accepted_here() {
    // P supersedes Q and changes only `scope.abstraction_level`, and `scope` is `unlocked`
    // in Q's policy, so the import admits it.
    // The bundle's acceptance of P is honored only once Q is this daemon's accepted head,
    // and accepting P then supersedes Q (A2, RFC 0037 R6).
    let mut exporter = with_entropy(10);
    let (_, identity) = exporter.mint(&[SignedArtifactKind::IntentBundle]);
    let q = exporter.put_accepted(&contract(None), None);
    let scoped = {
        let text = DIE_HARD_CONTRACT.trim_end().replacen(
            "\"abstraction_level\":null",
            "\"abstraction_level\":\"variant\"",
            1,
        );
        let unstamped = IntentContract::decode(text.as_bytes()).expect("a scope variant decodes");
        let handle = intent_handle(&unstamped);
        IntentContract::decode(
            text.replacen(
                "\"intent_id\":\"in_die_hard_v1\"",
                &format!("\"intent_id\":\"{}\"", handle.as_str()),
                1,
            )
            .as_bytes(),
        )
        .expect("stamped")
    };
    let p = exporter.put_accepted(&scoped, Some(q.clone()));
    assert_ne!(p, q);
    let (bundle, content) = exporter.export(vec![q.clone(), p.clone()]);
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(AllowedSigners::new().allow(identity, BUNDLE_KINDS)),
        ..Setup::default()
    });
    let (outcome, _, entered) = import_outcome(&importer.import(content));
    assert_eq!(outcome, SignatureOutcome::Verified);
    assert_eq!(entered, 2);
    // P first: its predecessor is only proposed here.
    assert_eq!(
        importer.accept(&p, Some(&bundle), FROM_BUNDLE).error_code(),
        Some(ErrorCode::AcceptanceChainInvalid)
    );
    assert_eq!(
        importer.accept(&q, Some(&bundle), FROM_BUNDLE).error_code(),
        None
    );
    assert_eq!(
        importer.accept(&p, Some(&bundle), FROM_BUNDLE).error_code(),
        None
    );
    let q_record = importer.daemon.state().intent(&q).expect("held");
    assert_eq!(q_record.status, RegistryStatus::Superseded);
    assert_eq!(q_record.superseded_by.as_ref(), Some(&p));
}

#[test]
fn the_ci_check_refuses_a_retired_signer() {
    // A rotated key's old signatures still verify, but a retired key does not vouch for a
    // new acceptance.
    let mut exporter = with_entropy(10);
    let (first_name, first) = exporter.mint(&[SignedArtifactKind::IntentBundle]);
    let proposal = exporter.put_accepted(&contract(None), None);
    let (old_bundle, old_content) = exporter.export(vec![proposal.clone()]);
    let rotated = exporter.call(
        STEWARD,
        Arguments::SigningRotate(SigningRotateRequest { signer: first_name }),
    );
    let Payload::SigningRotate(rotation) = rotated.payload else {
        panic!("rotate failed");
    };
    let second = SignerIdentity::from_public_key(&rotation.public_key).expect("a key");
    let other = exporter.put_accepted(&contract(Some(4)), None);
    let (_, new_content) = exporter.export(vec![other]);

    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(
            AllowedSigners::new()
                .allow(first.clone(), BUNDLE_KINDS)
                .allow(second, BUNDLE_KINDS),
        ),
        ..Setup::default()
    });
    let (outcome, adopted, _) = import_outcome(&importer.import(new_content));
    assert_eq!(outcome, SignatureOutcome::Verified);
    assert_eq!(adopted, 3, "two pinned mints and the rotation");
    assert!(matches!(
        importer
            .daemon
            .state()
            .signing()
            .registry()
            .standing(&first),
        Some(continuum_evidence::signing::SignerStanding::Rotated { .. })
    ));
    let (outcome, _, _) = import_outcome(&importer.import(old_content));
    assert_eq!(
        outcome,
        SignatureOutcome::Verified,
        "the old signature still verifies"
    );
    assert_eq!(
        importer
            .accept(&proposal, Some(&old_bundle), FROM_BUNDLE)
            .error_code(),
        Some(ErrorCode::AcceptanceChainInvalid),
        "but the retired signer does not vouch for an acceptance"
    );
}

#[test]
fn a_revocation_carried_by_a_bundle_is_adopted_and_a_stale_bundle_cannot_resurrect_the_key() {
    let mut exporter = with_entropy(10);
    let (old_name, old) = exporter.mint(&[SignedArtifactKind::IntentBundle]);
    let first = exporter.put_accepted(&contract(None), None);
    let (stale_bundle, stale_content) = exporter.export(vec![first.clone()]);
    // The exporter's key is compromised: rotate away from it, revoke it, export again. The
    // revocation is about a signer on the new signer's own chain, so it travels.
    let rotated = exporter.call(
        STEWARD,
        Arguments::SigningRotate(SigningRotateRequest {
            signer: old_name.clone(),
        }),
    );
    let Payload::SigningRotate(rotation) = rotated.payload else {
        panic!("rotate failed");
    };
    let new = SignerIdentity::from_public_key(&rotation.public_key).expect("a key");
    let revoked = exporter.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: old_name,
            reason: RevocationReason::Compromised,
        }),
    );
    assert_eq!(revoked.error_code(), None);
    let second = exporter.put_accepted(&contract(Some(3)), None);
    let (_, fresh_content) = exporter.export(vec![second]);

    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(
            AllowedSigners::new()
                .allow(old.clone(), BUNDLE_KINDS)
                .allow(new, BUNDLE_KINDS),
        ),
        ..Setup::default()
    });
    let (outcome, adopted, _) = import_outcome(&importer.import(fresh_content));
    assert_eq!(outcome, SignatureOutcome::Verified);
    assert_eq!(adopted, 4, "two mints, the rotation, and the revocation");
    assert!(matches!(
        importer.daemon.state().signing().registry().standing(&old),
        Some(continuum_evidence::signing::SignerStanding::Revoked { .. })
    ));
    // The stale bundle's log lacks the revocation, and the importer's standing decides: it
    // verifies as revoked, enters nothing, and accepts nothing.
    let (outcome, adopted, entered) = import_outcome(&importer.import(stale_content));
    assert_eq!(outcome, SignatureOutcome::SignerRevoked);
    assert_eq!((adopted, entered), (0, 0));
    assert_eq!(
        importer
            .accept(&first, Some(&stale_bundle), FROM_BUNDLE)
            .error_code(),
        Some(ErrorCode::AcceptanceChainInvalid)
    );
}

/// A bundle built by hand, carrying no links, signed by `key` (active in `registry`).
fn forged_bundle(
    registry: &SigningRegistry,
    key: &continuum_evidence::signing::LocalSigner,
    contracts: Vec<BundleContract>,
    allowed: AllowedSigners,
) -> Vec<u8> {
    linked_bundle(registry, key, contracts, allowed, Vec::new())
}

/// A bundle built by hand, carrying `links`, signed by `key` (active in `registry`).
fn linked_bundle(
    registry: &SigningRegistry,
    key: &continuum_evidence::signing::LocalSigner,
    contracts: Vec<BundleContract>,
    allowed: AllowedSigners,
    links: Vec<SignerLink>,
) -> Vec<u8> {
    let body = BundleBody {
        allowed,
        contracts,
        links,
    };
    let body_bytes = body.encode();
    let signature = registry
        .sign(key, Kind::IntentBundle, &signed_bytes_identity(&body_bytes))
        .expect("sign");
    encode_signed(&body_bytes, &signature.encode())
}

/// A key from one fixed seed byte, and a registry that knows it.
fn keyed(seed: u8) -> (SigningRegistry, continuum_evidence::signing::LocalSigner) {
    let mut registry = SigningRegistry::new();
    let actor = SigningActor::new("human:peer").expect("actor");
    let key = registry
        .mint(&actor, &mut OneSeed([seed; SEED_LEN]))
        .expect("mint");
    (registry, key)
}

/// A frame list, as `rule intent.bundles` spells one.
fn frames(items: &[&[u8]]) -> Vec<u8> {
    let mut out = u32::try_from(items.len())
        .expect("small")
        .to_be_bytes()
        .to_vec();
    for item in items {
        out.extend_from_slice(&u32::try_from(item.len()).expect("small").to_be_bytes());
        out.extend_from_slice(item);
    }
    out
}

#[test]
fn truncated_extended_or_oversize_bundles_are_refused_whole() {
    let (mut exporter, mut importer, proposal, _) = exporter_and_importer();
    let (_, content) = exporter.export(vec![proposal.clone()]);
    let step = (content.len() / 97).max(1);
    for cut in (0..content.len()).step_by(step) {
        let outcome = importer.import(content[..cut].to_vec());
        assert_eq!(
            outcome.error_code(),
            Some(ErrorCode::MalformedRequest),
            "cut at {cut}"
        );
    }
    let mut trailing = content.clone();
    trailing.push(0);
    assert_eq!(
        importer.import(trailing).error_code(),
        Some(ErrorCode::MalformedRequest)
    );
    let over = vec![0u8; MAX_BUNDLE_LEN + 1];
    assert_eq!(
        importer.import(over).error_code(),
        Some(ErrorCode::MalformedRequest)
    );
    // A frame count far past the bytes left, inside an otherwise well-shaped body.
    let signed = decode_signed(&content).expect("reads");
    let mut hostile_list = u32::MAX.to_be_bytes().to_vec();
    hostile_list.extend_from_slice(&[0, 0, 0, 0]);
    let hostile_body = frames(&[
        b"continuum.intent-bundle.v1",
        &hostile_list,
        &frames(&[]),
        &frames(&[]),
    ]);
    let hostile = encode_signed(&hostile_body, signed.signature());
    assert_eq!(
        importer.import(hostile).error_code(),
        Some(ErrorCode::MalformedRequest)
    );
    // A top layer that is a general value rather than a frame list is refused on its shape.
    let not_frames =
        continuum_value::value::Value::seq((0..1000).map(continuum_value::value::Value::Nat))
            .expect("seq")
            .encode();
    assert_eq!(
        importer.import(not_frames).error_code(),
        Some(ErrorCode::MalformedRequest)
    );
    // Nothing of any refused bundle reached the registry, the intents, or the held set.
    assert_eq!(importer.records(), 0);
    assert!(importer.daemon.state().intent(&proposal).is_none());
}

#[test]
fn a_bundle_whose_contract_is_not_its_declared_identity_is_refused() {
    let mut registry = SigningRegistry::new();
    let key = registry
        .mint(
            &SigningActor::new("human:x").expect("actor"),
            &mut Seeds(90),
        )
        .expect("mint");
    let allowed = AllowedSigners::new().allow(key.identity().clone(), BUNDLE_KINDS);
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(allowed.clone()),
        ..Setup::default()
    });
    let real = contract(None);
    let lie = IntentHandle::new("in_notthisone").expect("an in_");
    let record = format!(
        "{{\"intent\":\"{}\",\"record_id\":\"{}\",\"schema_epoch\":1,\"schema_id\":\"https://continuum.dev/schema/intent-registry-record.json\",\"status\":\"proposed\",\"superseded_by\":null,\"supersedes\":null}}",
        lie.as_str(),
        lie.as_str()
    );
    let bytes = forged_bundle(
        &registry,
        &key,
        vec![BundleContract {
            intent: lie.clone(),
            contract: real.to_artifact_bytes(),
            record: record.into_bytes(),
        }],
        allowed,
    );
    assert_eq!(
        importer.import(bytes).error_code(),
        Some(ErrorCode::MalformedRequest)
    );
    assert!(importer.daemon.state().intent(&lie).is_none());
    assert_eq!(importer.records(), 0);
}

#[test]
fn import_is_idempotent_and_never_raises_status() {
    let (mut exporter, mut importer, proposal, _) = exporter_and_importer();
    let (bundle, content) = exporter.export(vec![proposal.clone()]);
    let _ = importer.import(content.clone());
    let records = importer.records();
    let second = importer.import(content);
    let Payload::IntentImportBundle(again) = &second.payload else {
        panic!("re-import failed");
    };
    assert_eq!(again.bundle, bundle);
    assert_eq!(again.adopted, 0);
    assert!(again.imported.is_empty());
    assert_eq!(importer.records(), records);
    assert_eq!(
        second.envelope.verdict,
        Nullable::Value(continuumd::protocol::envelope::Verdict::Structural(
            continuumd::protocol::envelope::StructuralVerdictValue {
                outcome: StructuralOutcome::Unchanged
            }
        ))
    );
    assert_eq!(
        importer
            .daemon
            .state()
            .intent(&proposal)
            .expect("held")
            .status,
        RegistryStatus::Proposed
    );
}

#[test]
fn a_full_registry_can_still_revoke() {
    // A registry at the mint bound refuses a mint and a rotation, and still revokes: the
    // reserve is kept for the compromise answer.
    let actor = SigningActor::new("human:solo-dev").expect("actor");
    let mut registry = SigningRegistry::new();
    let key = registry.mint(&actor, &mut Seeds(1)).expect("mint");
    let mut others = Vec::new();
    let mut n: u32 = 0;
    while registry.audit_log().len()
        < continuumd::daemon::bundle::MAX_AUDIT_RECORDS
            - continuumd::daemon::signing::REVOCATION_RESERVE
    {
        n += 1;
        let mut seed = [0u8; SEED_LEN];
        seed[..4].copy_from_slice(&n.to_le_bytes());
        seed[31] = 0xA5;
        let mut scratch = SigningRegistry::new();
        let other = scratch
            .mint(&actor, &mut OneSeed(seed))
            .expect("mint")
            .identity()
            .clone();
        registry
            .record_observed(
                &actor,
                continuum_evidence::signing::SigningEvent::Minted {
                    signer: other.clone(),
                },
            )
            .expect("a new signer");
        others.push(other);
    }
    let mut world = world(Setup {
        entropy: Some(200),
        signer: Some(ReceiptSigner::new(registry, key).expect("installable")),
        ..Setup::default()
    });
    let held = world
        .daemon
        .state()
        .signing()
        .held()
        .cloned()
        .expect("held");
    let rotate = world.call(
        STEWARD,
        Arguments::SigningRotate(SigningRotateRequest {
            signer: signer_handle(&held),
        }),
    );
    assert_eq!(rotate.error_code(), Some(ErrorCode::QuotaExhausted));
    let revoke = world.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: signer_handle(&others[0]),
            reason: RevocationReason::Compromised,
        }),
    );
    assert_eq!(revoke.error_code(), None, "a revocation uses the reserve");
    // Revocations of other signers stop short of the held key's own reserve…
    let mut next = 1;
    while world.records()
        < continuumd::daemon::bundle::MAX_AUDIT_RECORDS
            - continuumd::daemon::signing::HELD_KEY_RESERVE
    {
        let revoke = world.call(
            STEWARD,
            Arguments::SigningRevoke(SigningRevokeRequest {
                signer: signer_handle(&others[next]),
                reason: RevocationReason::Compromised,
            }),
        );
        assert_eq!(revoke.error_code(), None);
        next += 1;
    }
    let refused = world.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: signer_handle(&others[next]),
            reason: RevocationReason::Compromised,
        }),
    );
    assert_eq!(refused.error_code(), Some(ErrorCode::QuotaExhausted));
    // …so the held key can still be recovered as lost, which records three.
    let recovered = world.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: signer_handle(&held),
            reason: RevocationReason::KeyLost,
        }),
    );
    assert_eq!(
        recovered.error_code(),
        None,
        "loss recovery uses the held key's reserve"
    );
}

/// One fixed seed.
struct OneSeed([u8; SEED_LEN]);

impl KeyEntropy for OneSeed {
    fn seed(&mut self) -> Result<Zeroizing<[u8; SEED_LEN]>, EntropyUnavailable> {
        Ok(Zeroizing::new(self.0))
    }
}

#[test]
fn a_mint_signs_only_the_kinds_it_names() {
    let mut world = with_entropy(1);
    let duplicate = world.call(
        STEWARD,
        Arguments::SigningMint(SigningMintRequest {
            kinds: vec![SignedArtifactKind::Receipt, SignedArtifactKind::Receipt],
        }),
    );
    assert_eq!(duplicate.error_code(), Some(ErrorCode::MalformedRequest));
    let _ = world.mint(&[SignedArtifactKind::DomainPack]);
    let proposal = world.put_accepted(&contract(None), None);
    let export = world.call(
        STEWARD,
        Arguments::IntentExportBundle(IntentExportBundleRequest {
            intents: vec![proposal],
        }),
    );
    assert_eq!(
        export.error_code(),
        Some(ErrorCode::PolicyGateFailed),
        "a pack key signs no bundle"
    );
    let pack = world.call(
        STEWARD,
        Arguments::SigningSignPack(SigningSignPackRequest {
            pack: b"p".to_vec(),
        }),
    );
    assert_eq!(pack.error_code(), None);
}

/// Every `(operation, code)` pair the signing family and the two bundle operations can
/// answer with is inside `rule errors.common` and the operation's own `errors` clause.
#[test]
fn every_code_the_signing_wire_answers_with_is_declared() {
    let common = [
        ErrorCode::MalformedRequest,
        ErrorCode::ProtocolVersionUnsupported,
        ErrorCode::CapabilityDenied,
        ErrorCode::QuotaExhausted,
        ErrorCode::EpochUnsupported,
        ErrorCode::UnsupportedSemanticFeature,
    ];
    let mutation = [
        ErrorCode::IdempotencyKeyReused,
        ErrorCode::PublicationAborted,
    ];
    let intent = continuumd::daemon::intent::FAULTS
        .iter()
        .filter(|(operation, _)| operation.ends_with("_bundle"));
    let mut checked = 0;
    for (operation, code) in continuumd::daemon::signing::FAULTS.iter().chain(intent) {
        let spec = registry::operation(operation).expect("the operation is registered");
        let allowed = common.contains(code)
            || (spec.has(Annotation::Mutation) && mutation.contains(code))
            || spec.errors.contains(code);
        assert!(allowed, "`{operation}` may not answer with {code:?}");
        checked += 1;
    }
    assert!(checked >= 20, "the tables are not empty");
}

const IDL: &str = include_str!("../../../notes/plan/schemas/continuumd-native-protocol.idl");

#[test]
fn the_version_gate_is_exactly_the_idls_3_8_operations() {
    // Every `operation NAME {` the IDL dates `@since("3.8")` on the line above.
    let lines: Vec<&str> = IDL.lines().collect();
    let dated: Vec<&str> = lines
        .windows(2)
        .filter(|pair| pair[0].trim() == "@since(\"3.8\")")
        .filter_map(|pair| {
            pair[1]
                .trim()
                .strip_prefix("operation ")
                .and_then(|rest| rest.strip_suffix(" {"))
        })
        .collect();
    assert_eq!(
        dated.len(),
        8,
        "six signing operations and two bundle operations"
    );
    for spec in registry::OPERATIONS {
        let gated = registry::introduced_at(spec.name);
        if dated.contains(&spec.name) {
            assert_eq!(gated, Some(v38()), "{} is dated 3.8 in the IDL", spec.name);
        } else {
            assert_eq!(gated, None, "{} is not dated 3.8 in the IDL", spec.name);
        }
    }
}

#[test]
fn an_export_pins_only_the_active_bundle_signers() {
    // Rotations retire keys; the pinned set of an export does not grow with them.
    let mut exporter = with_entropy(10);
    let (mut name, _) = exporter.mint(&[SignedArtifactKind::IntentBundle]);
    for _ in 0..3 {
        let rotated = exporter.call(
            STEWARD,
            Arguments::SigningRotate(SigningRotateRequest { signer: name }),
        );
        let Payload::SigningRotate(rotation) = rotated.payload else {
            panic!("rotate failed");
        };
        name = rotation.successor;
    }
    assert_eq!(exporter.daemon.state().signing().allowed().len(), 4);
    let proposal = exporter.put_accepted(&contract(None), None);
    let (_, content) = exporter.export(vec![proposal]);
    let signed = decode_signed(&content).expect("a bundle");
    let pinned: Vec<_> = signed
        .body()
        .allowed
        .iter()
        .map(|(signer, _)| signer.clone())
        .collect();
    let held = exporter
        .daemon
        .state()
        .signing()
        .held()
        .cloned()
        .expect("held");
    assert_eq!(pinned, vec![held]);
}

// --- identity collisions (cr-2unxyh) ------------------------------------------------------

/// An identity seam that names every body of the chosen classes alike, as the public
/// builder seam permits: the injected collision ADR-0013 says must resolve by exact
/// comparison. Every other class is BLAKE3.
#[derive(Debug, Clone, Copy)]
struct Colliding {
    intents: bool,
    bundles: bool,
    /// Every receipt node checked under the `kernel-core/1` profile.
    receipts: bool,
}

impl continuum_workspace::publication::ContentIdentifier for Colliding {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<
        continuum_workspace::artifact_path::ArtifactHandle,
        continuum_workspace::publication::IdentityUnavailable,
    > {
        let collide = (self.intents && class == ArtifactClass::IntentContract)
            || (self.bundles && class == ArtifactClass::SignedIntentBundle)
            || (self.receipts
                && class == ArtifactClass::Evidence
                && content.ends_with(b"kernel-core/1"));
        let bytes: &[u8] = if collide {
            b"one identity for every body"
        } else {
            content
        };
        continuum_workspace::publication::ContentIdentifier::identify(&Blake3Identity, class, bytes)
    }
}

/// The handle the colliding seam gives every contract.
fn colliding_intent() -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Colliding {
            intents: true,
            bundles: false,
            receipts: false,
        },
        ArtifactClass::IntentContract,
        b"",
    )
    .expect("an identity");
    continuumd::daemon::identity::intent_to_wire(&stored).expect("an in_ handle")
}

/// Die-hard variant `faults`, stamped with `handle` as its `intent_id`.
fn contract_named(faults: u32, handle: &IntentHandle) -> IntentContract {
    let text = DIE_HARD_CONTRACT
        .trim_end()
        .replace("\"faults\":0", &format!("\"faults\":{faults}"))
        .replacen(
            "\"intent_id\":\"in_die_hard_v1\"",
            &format!("\"intent_id\":\"{}\"", handle.as_str()),
            1,
        );
    IntentContract::decode(text.as_bytes()).expect("the stamped fixture decodes")
}

fn colliding_world(entropy: u8, collide: Colliding, allowed: Option<AllowedSigners>) -> World {
    world(Setup {
        entropy: Some(entropy),
        allowed,
        collide: Some(collide),
        ..Setup::default()
    })
}

impl World {
    fn put_named(
        &mut self,
        handle: &IntentHandle,
        contract: IntentContract,
        status: RegistryStatus,
    ) {
        self.daemon.state_mut().put_intent(
            handle.clone(),
            IntentRecord {
                contract,
                status: RegistryStatus::Proposed,
                supersedes: None,
                superseded_by: None,
                acceptance: None,
            },
        );
        if status == RegistryStatus::Accepted {
            let accepted = self.accept(handle, None, CALLER_TEXT);
            assert_eq!(accepted.error_code(), None, "a local acceptance");
        }
    }

    fn collisions(&self) -> u64 {
        self.daemon.state().signing().identity_collisions()
    }
}

const INTENTS: Colliding = Colliding {
    intents: true,
    bundles: false,
    receipts: false,
};
const BUNDLES: Colliding = Colliding {
    intents: false,
    bundles: true,
    receipts: false,
};

#[test]
fn an_import_whose_contract_collides_with_a_held_one_is_refused_before_anything_changes() {
    let handle = colliding_intent();
    let mut exporter = colliding_world(10, INTENTS, None);
    let (_, key) = exporter.mint(&[SignedArtifactKind::IntentBundle]);
    exporter.put_named(
        &handle,
        contract_named(7, &handle),
        RegistryStatus::Accepted,
    );
    let (bundle, content) = exporter.export(vec![handle.clone()]);
    let pins = AllowedSigners::new().allow(key, BUNDLE_KINDS);
    // Held locally under the same handle, proposed and accepted alike: body A, not B.
    for status in [RegistryStatus::Proposed, RegistryStatus::Accepted] {
        let mut importer = colliding_world(100, INTENTS, Some(pins.clone()));
        let local = contract_named(9, &handle);
        importer.put_named(&handle, local.clone(), status);
        let registry = importer.daemon.state().signing().registry().clone();
        let refused = importer.import(content.clone());
        assert_eq!(refused.error_code(), Some(ErrorCode::PublicationAborted));
        let state = importer.daemon.state();
        assert_eq!(state.signing().registry(), &registry, "no fact adopted");
        assert!(state.signing().bundle(&bundle).is_none(), "nothing held");
        assert!(state.signing().imported_from(&handle).is_none());
        let held = state.intent(&handle).expect("still held");
        assert_eq!(held.contract.to_artifact_bytes(), local.to_artifact_bytes());
        assert_eq!(held.status, status);
        assert_eq!(importer.collisions(), 1);
        // The detail names the collision and carries no contract bytes.
        let detail = format!("{:?}", refused.envelope.error);
        assert!(
            !detail.contains("die"),
            "no contract material in the refusal"
        );
    }
    // The same bytes, where nothing collides, import and re-import idempotently.
    let mut importer = colliding_world(100, INTENTS, Some(pins));
    let (outcome, _, entered) = import_outcome(&importer.import(content.clone()));
    assert_eq!((outcome, entered), (SignatureOutcome::Verified, 1));
    let (outcome, _, entered) = import_outcome(&importer.import(content));
    assert_eq!((outcome, entered), (SignatureOutcome::Verified, 0));
    assert_eq!(importer.collisions(), 0);
}

#[test]
fn accept_never_takes_a_signature_over_one_body_for_another_under_the_same_identity() {
    let handle = colliding_intent();
    let mut exporter = colliding_world(10, INTENTS, None);
    let (_, key) = exporter.mint(&[SignedArtifactKind::IntentBundle]);
    exporter.put_named(
        &handle,
        contract_named(7, &handle),
        RegistryStatus::Accepted,
    );
    let (bundle, content) = exporter.export(vec![handle.clone()]);
    let pins = AllowedSigners::new().allow(key, BUNDLE_KINDS);

    // Control: the bundle over B accepts B.
    let mut control = colliding_world(100, INTENTS, Some(pins.clone()));
    let _ = control.import(content.clone());
    assert_eq!(
        control
            .accept(&handle, Some(&bundle), FROM_BUNDLE)
            .error_code(),
        None
    );

    // The same bundle is held, but the local proposal under that identity is body A.
    let mut importer = colliding_world(100, INTENTS, Some(pins));
    let _ = importer.import(content);
    importer.put_named(
        &handle,
        contract_named(9, &handle),
        RegistryStatus::Proposed,
    );
    let refused = importer.accept(&handle, Some(&bundle), FROM_BUNDLE);
    assert_eq!(
        refused.error_code(),
        Some(ErrorCode::AcceptanceChainInvalid)
    );
    assert_eq!(
        importer
            .daemon
            .state()
            .intent(&handle)
            .expect("held")
            .status,
        RegistryStatus::Proposed
    );
    assert_eq!(importer.collisions(), 1);
}

#[test]
fn a_bundle_identity_that_names_other_bytes_is_refused_on_import_and_export() {
    let mut exporter = colliding_world(10, BUNDLES, None);
    let (_, key) = exporter.mint(&[SignedArtifactKind::IntentBundle]);
    let first = exporter.put_accepted(&contract(None), None);
    let second = exporter.put_accepted(&contract(Some(3)), None);
    let (bundle, x) = exporter.export(vec![first.clone()]);
    // Export: the held `inb_` already names X, so a different export under it is refused.
    let refused = exporter.call(
        STEWARD,
        Arguments::IntentExportBundle(IntentExportBundleRequest {
            intents: vec![first.clone(), second.clone()],
        }),
    );
    assert_eq!(refused.error_code(), Some(ErrorCode::PublicationAborted));
    assert_eq!(exporter.collisions(), 1);
    // Y: the same key and contracts from a twin that holds no bundle yet.
    let mut twin = colliding_world(10, BUNDLES, None);
    let _ = twin.mint(&[SignedArtifactKind::IntentBundle]);
    twin.put_accepted(&contract(None), None);
    twin.put_accepted(&contract(Some(3)), None);
    let (twin_bundle, y) = twin.export(vec![first.clone(), second.clone()]);
    assert_eq!(twin_bundle, bundle, "the seam names X and Y alike");
    assert_ne!(x, y);

    let pins = AllowedSigners::new().allow(key, BUNDLE_KINDS);
    let mut importer = colliding_world(100, BUNDLES, Some(pins));
    let (outcome, _, _) = import_outcome(&importer.import(x.clone()));
    assert_eq!(outcome, SignatureOutcome::Verified);
    let registry = importer.daemon.state().signing().registry().clone();
    let refused = importer.import(y);
    assert_eq!(refused.error_code(), Some(ErrorCode::PublicationAborted));
    let state = importer.daemon.state();
    assert_eq!(state.signing().registry(), &registry);
    assert!(
        state.intent(&second).is_none(),
        "Y's contract did not enter"
    );
    assert_eq!(
        state
            .signing()
            .bundle(&bundle)
            .map(|held| held.body_bytes().to_vec()),
        decode_signed(&x)
            .ok()
            .map(|signed| signed.body_bytes().to_vec()),
        "X is still the bundle held"
    );
    assert_eq!(importer.collisions(), 1);
    // Re-importing X's exact bytes stays idempotent.
    let (outcome, adopted, entered) = import_outcome(&importer.import(x));
    assert_eq!(
        (outcome, adopted, entered),
        (SignatureOutcome::Verified, 0, 0)
    );
    assert_eq!(importer.collisions(), 1);
}

#[test]
fn a_receipt_node_identity_that_names_another_receipt_is_refused_before_signing() {
    let mut registry = SigningRegistry::new();
    let key = registry
        .mint(
            &SigningActor::new("human:solo-dev").expect("actor"),
            &mut Seeds(7),
        )
        .expect("mint");
    let mut world = world(Setup {
        signer: Some(ReceiptSigner::new(registry, key).expect("installable")),
        collide: Some(Colliding {
            intents: false,
            bundles: false,
            receipts: true,
        }),
        ..Setup::default()
    });
    let stage = |daemon: &mut Daemon, path: &str, content: &[u8]| {
        daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(path).expect("path"),
                content.to_vec(),
            )
            .expect("staged")
    };
    let trace = stage(&mut world.daemon, "traces/t.jsonl", b"{\"events\":[]}\n");
    let first = b"{\"receipt\":\"first\"}\n".to_vec();
    let one = stage(&mut world.daemon, "receipts/one.json", &first);
    let two = stage(
        &mut world.daemon,
        "receipts/two.json",
        b"{\"receipt\":\"second\"}\n",
    );
    let ingested = world.call(
        PRODUCER,
        Arguments::ObserveIngest(ObserveIngestRequest {
            trace,
            instrumentation_profile: "otel-1.0/sampled".to_owned(),
        }),
    );
    let Payload::ObserveIngest(ingest) = ingested.payload else {
        panic!("ingest failed: {:?}", ingested.envelope.error);
    };
    let link = |world: &mut World, receipt| {
        world.call(
            CHECKER,
            Arguments::EvidenceLink(EvidenceLinkRequest {
                subject: ingest.evidence[0].clone(),
                receipt,
                checker_profile: "kernel-core/1".to_owned(),
            }),
        )
    };
    let linked = link(&mut world, one.clone());
    let Payload::EvidenceLink(linked) = linked.payload else {
        panic!("link failed: {:?}", linked.envelope.error);
    };
    let refused = link(&mut world, two);
    assert_eq!(refused.error_code(), Some(ErrorCode::PublicationAborted));
    assert_eq!(world.collisions(), 1);
    // The node keeps the first receipt's signature, which still verifies over it.
    let got = world.call(
        READER,
        Arguments::EvidenceGet(EvidenceGetRequest {
            evidence: linked.receipt.clone(),
            inline: Optional::Absent,
        }),
    );
    let Payload::EvidenceGet(node) = got.payload else {
        panic!("evidence.get failed");
    };
    let Optional::Present(signature) = node.signature else {
        panic!("signed");
    };
    assert_eq!(
        world.verify(SignedArtifactKind::Receipt, &first, Some(signature)),
        SignatureOutcome::Verified
    );
    // Linking the same receipt again is not a collision.
    assert_eq!(link(&mut world, one).error_code(), None);
    assert_eq!(world.collisions(), 1);
}

// --- key-attested lineage (cr-2unxyh round 2) -----------------------------------------------

fn standing(
    world: &World,
    signer: &SignerIdentity,
) -> Option<continuum_evidence::signing::SignerStanding> {
    world
        .daemon
        .state()
        .signing()
        .registry()
        .standing(signer)
        .cloned()
}

fn is_revoked(standing: Option<continuum_evidence::signing::SignerStanding>) -> bool {
    matches!(
        standing,
        Some(continuum_evidence::signing::SignerStanding::Revoked { .. })
    )
}

#[test]
fn a_known_signer_learns_its_attested_rotation_and_revocation() {
    // The importer first learns A from A's own bundle. The exporter then rotates A -> B and
    // revokes A as compromised; B's bundle carries both links, each signed by A (and B).
    let mut exporter = with_entropy(10);
    let (a_name, a) = exporter.mint(&[SignedArtifactKind::IntentBundle]);
    let first = exporter.put_accepted(&contract(None), None);
    let (a_bundle, a_content) = exporter.export(vec![first.clone()]);
    let rotated = exporter.call(
        STEWARD,
        Arguments::SigningRotate(SigningRotateRequest {
            signer: a_name.clone(),
        }),
    );
    let Payload::SigningRotate(rotation) = rotated.payload else {
        panic!("rotate failed");
    };
    let b = SignerIdentity::from_public_key(&rotation.public_key).expect("a key");
    let revoked = exporter.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: a_name,
            reason: RevocationReason::Compromised,
        }),
    );
    assert_eq!(revoked.error_code(), None);
    let second = exporter.put_accepted(&contract(Some(3)), None);
    let (_, b_content) = exporter.export(vec![second]);

    let pins = AllowedSigners::new()
        .allow(a.clone(), BUNDLE_KINDS)
        .allow(b.clone(), BUNDLE_KINDS);
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(pins),
        ..Setup::default()
    });
    let (outcome, adopted, _) = import_outcome(&importer.import(a_content));
    assert_eq!((outcome, adopted), (SignatureOutcome::Verified, 1));
    assert_eq!(
        standing(&importer, &a),
        Some(continuum_evidence::signing::SignerStanding::Active)
    );
    let (outcome, adopted, _) = import_outcome(&importer.import(b_content.clone()));
    assert_eq!(outcome, SignatureOutcome::Verified);
    assert_eq!(
        adopted, 3,
        "B's mint, the rotation A -> B, and A's revocation"
    );
    assert!(is_revoked(standing(&importer, &a)));
    assert_eq!(
        standing(&importer, &b),
        Some(continuum_evidence::signing::SignerStanding::Active)
    );
    // The compromised key no longer passes the CI check.
    assert_eq!(
        importer
            .accept(&first, Some(&a_bundle), FROM_BUNDLE)
            .error_code(),
        Some(ErrorCode::AcceptanceChainInvalid)
    );
    // Replay adopts nothing.
    let (_, adopted, _) = import_outcome(&importer.import(b_content));
    assert_eq!(adopted, 0);
}

#[test]
fn a_signer_cannot_claim_a_known_or_unseen_pinned_key_without_its_signature() {
    use continuum_evidence::signing::LinkEvent;
    let (a_log, ka) = keyed(140);
    let (b_log, kb) = keyed(150);
    let (_, kb2) = keyed(151);
    let (a, b) = (ka.identity().clone(), kb.identity().clone());
    let pins = AllowedSigners::new()
        .allow(a.clone(), BUNDLE_KINDS)
        .allow(b.clone(), BUNDLE_KINDS);
    // Signatures B can make, reused where A's are required.
    let own = rotation_link(&kb, &kb2);
    let b_signature = own.signatures()[0];
    let forged = [
        SignerLink::from_parts(
            LinkEvent::Rotated {
                from: a.clone(),
                to: b.clone(),
            },
            vec![b_signature, b_signature],
        ),
        SignerLink::from_parts(LinkEvent::Revoked { signer: a.clone() }, vec![b_signature]),
    ];
    for known in [true, false] {
        for link in &forged {
            let mut importer = world(Setup {
                entropy: Some(100),
                allowed: Some(pins.clone()),
                ..Setup::default()
            });
            if known {
                let bytes = forged_bundle(&a_log, &ka, Vec::new(), pins.clone());
                assert_eq!(import_outcome(&importer.import(bytes)).1, 1);
            }
            let before = importer.daemon.state().signing().registry().clone();
            let bytes = linked_bundle(&b_log, &kb, Vec::new(), pins.clone(), vec![link.clone()]);
            let refused = importer.import(bytes);
            assert_eq!(refused.error_code(), Some(ErrorCode::MalformedRequest));
            assert_eq!(importer.daemon.state().signing().registry(), &before);
            assert_eq!(importer.daemon.state().signing().unattested_links(), 1);
            let expected = known.then_some(continuum_evidence::signing::SignerStanding::Active);
            assert_eq!(standing(&importer, &a), expected, "A untouched");
            assert!(standing(&importer, &b).is_none(), "B not introduced either");
        }
    }
}

#[test]
fn a_link_about_this_daemons_own_key_is_never_adopted() {
    // The importer mints KA (seed 101) and rotates it. A thief holding the retired KA and a
    // pinned peer key KB attests KA's revocation: the link verifies, and is still skipped.
    let (b_log, kb) = keyed(150);
    let (_, x_key) = keyed(152);
    let pins = AllowedSigners::new()
        .allow(kb.identity().clone(), BUNDLE_KINDS)
        .allow(x_key.identity().clone(), BUNDLE_KINDS);
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(pins.clone()),
        ..Setup::default()
    });
    let (name, retired) = importer.mint(&[SignedArtifactKind::Receipt]);
    let (_, stolen) = keyed(101);
    assert_eq!(
        stolen.identity(),
        &retired,
        "the same seed makes the same key"
    );
    let rotated = importer.call(
        STEWARD,
        Arguments::SigningRotate(SigningRotateRequest { signer: name }),
    );
    assert_eq!(rotated.error_code(), None);
    let (_, x) = keyed(152);
    let links = vec![
        revocation_link(&stolen),
        knowing(&[&stolen, &x])
            .attest_rotation(&stolen, &x)
            .expect("active in the thief's registry"),
    ];
    let bytes = linked_bundle(&b_log, &kb, Vec::new(), pins, links);
    let (outcome, adopted, _) = import_outcome(&importer.import(bytes));
    assert_eq!(outcome, SignatureOutcome::Verified);
    assert_eq!(adopted, 1, "only KB's own mint");
    assert!(matches!(
        standing(&importer, &retired),
        Some(continuum_evidence::signing::SignerStanding::Rotated { .. })
    ));
}

#[test]
fn a_retired_key_adopts_no_facts() {
    // The importer learns K1 -> K2. K2's next bundle carries K1 -> X, attested by K1 (now
    // leaked) and X, with X pinned: the link is genuine, and K1 is already retired.
    let (_, k1) = keyed(150);
    let (k2_log, k2) = keyed(151);
    let (_, x) = keyed(152);
    let pins = AllowedSigners::new()
        .allow(k1.identity().clone(), BUNDLE_KINDS)
        .allow(k2.identity().clone(), BUNDLE_KINDS)
        .allow(x.identity().clone(), BUNDLE_KINDS);
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(pins.clone()),
        ..Setup::default()
    });
    let bytes = linked_bundle(
        &k2_log,
        &k2,
        Vec::new(),
        pins.clone(),
        vec![rotation_link(&k1, &k2)],
    );
    let (outcome, adopted, _) = import_outcome(&importer.import(bytes));
    assert_eq!((outcome, adopted), (SignatureOutcome::Verified, 3));
    let before = importer.daemon.state().signing().registry().clone();
    let bytes = linked_bundle(&k2_log, &k2, Vec::new(), pins, vec![rotation_link(&k1, &x)]);
    let (outcome, adopted, _) = import_outcome(&importer.import(bytes));
    assert_eq!(outcome, SignatureOutcome::Verified, "K2 is active");
    assert_eq!(
        adopted, 0,
        "K1 is already retired, so its second succession is refused"
    );
    assert_eq!(importer.daemon.state().signing().registry(), &before);
    assert!(standing(&importer, x.identity()).is_none());
}

#[test]
fn a_bundle_that_rotates_one_key_twice_is_refused_whole() {
    let (_, k1) = keyed(150);
    let (k2_log, k2) = keyed(151);
    let (_, x) = keyed(152);
    let pins = AllowedSigners::new()
        .allow(k1.identity().clone(), BUNDLE_KINDS)
        .allow(k2.identity().clone(), BUNDLE_KINDS)
        .allow(x.identity().clone(), BUNDLE_KINDS);
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(pins.clone()),
        ..Setup::default()
    });
    let bytes = linked_bundle(
        &k2_log,
        &k2,
        Vec::new(),
        pins,
        vec![rotation_link(&k1, &k2), rotation_link(&k1, &x)],
    );
    let refused = importer.import(bytes);
    assert_eq!(refused.error_code(), Some(ErrorCode::MalformedRequest));
    assert!(
        importer
            .daemon
            .state()
            .signing()
            .registry()
            .audit_log()
            .is_empty()
    );
    assert_eq!(importer.daemon.state().signing().unattested_links(), 1);
}

#[test]
fn an_unpinned_successor_is_never_introduced() {
    let (_, k1) = keyed(150);
    let (k1_log, k1_again) = keyed(150);
    let (_, stranger) = keyed(153);
    let pins = AllowedSigners::new().allow(k1.identity().clone(), BUNDLE_KINDS);
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(pins.clone()),
        ..Setup::default()
    });
    let bytes = linked_bundle(
        &k1_log,
        &k1_again,
        Vec::new(),
        pins,
        vec![rotation_link(&k1, &stranger)],
    );
    let (outcome, adopted, _) = import_outcome(&importer.import(bytes));
    assert_eq!(
        (outcome, adopted),
        (SignatureOutcome::Verified, 1),
        "only K1's own mint"
    );
    assert!(standing(&importer, stranger.identity()).is_none());
    assert_eq!(
        standing(&importer, k1.identity()),
        Some(continuum_evidence::signing::SignerStanding::Active)
    );
}

#[test]
fn link_order_and_replay_do_not_change_what_is_adopted() {
    let (_, ka) = keyed(140);
    let (b_log, kb) = keyed(150);
    let pins = AllowedSigners::new()
        .allow(ka.identity().clone(), BUNDLE_KINDS)
        .allow(kb.identity().clone(), BUNDLE_KINDS);
    let rotation = rotation_link(&ka, &kb);
    let revocation = revocation_link(&ka);
    let mut results = Vec::new();
    for links in [
        vec![rotation.clone(), revocation.clone()],
        vec![revocation, rotation],
    ] {
        let mut importer = world(Setup {
            entropy: Some(100),
            allowed: Some(pins.clone()),
            ..Setup::default()
        });
        let bytes = linked_bundle(&b_log, &kb, Vec::new(), pins.clone(), links);
        let (_, adopted, _) = import_outcome(&importer.import(bytes.clone()));
        assert_eq!(adopted, 4, "B, A, the rotation, and the revocation");
        assert_eq!(
            import_outcome(&importer.import(bytes)).1,
            0,
            "a replay adopts nothing"
        );
        let registry = importer.daemon.state().signing().registry();
        results.push((
            registry.standing(ka.identity()).cloned().map(|s| {
                matches!(
                    s,
                    continuum_evidence::signing::SignerStanding::Revoked { .. }
                )
            }),
            registry.standing(kb.identity()).cloned(),
            registry
                .audit_log()
                .iter()
                .map(|record| record.event().clone())
                .collect::<Vec<_>>(),
        ));
    }
    assert_eq!(results[0], results[1], "the carried order does not matter");
    assert_eq!(results[0].0, Some(true));
}

#[test]
fn a_loss_recovery_never_travels() {
    let mut exporter = with_entropy(10);
    let (a_name, a) = exporter.mint(&[SignedArtifactKind::IntentBundle]);
    let first = exporter.put_accepted(&contract(None), None);
    let (_, a_content) = exporter.export(vec![first]);
    let recovered = exporter.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: a_name,
            reason: RevocationReason::KeyLost,
        }),
    );
    let Payload::SigningRevoke(recovery) = recovered.payload else {
        panic!("recovery failed");
    };
    let Nullable::Value(b_name) = recovery.successor else {
        panic!("a successor");
    };
    let second = exporter.put_accepted(&contract(Some(3)), None);
    let (_, b_content) = exporter.export(vec![second]);
    let b = exporter
        .daemon
        .state()
        .signing()
        .held()
        .cloned()
        .expect("held");
    assert_eq!(signer_handle(&b), b_name);
    let pins = AllowedSigners::new()
        .allow(a.clone(), BUNDLE_KINDS)
        .allow(b.clone(), BUNDLE_KINDS);
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(pins),
        ..Setup::default()
    });
    let _ = importer.import(a_content);
    let (outcome, adopted, _) = import_outcome(&importer.import(b_content));
    assert_eq!(
        (outcome, adopted),
        (SignatureOutcome::Verified, 1),
        "only B"
    );
    assert_eq!(
        standing(&importer, &a),
        Some(continuum_evidence::signing::SignerStanding::Active),
        "a peer learns of a loss only from its own operator"
    );
}

#[test]
fn a_bundle_whose_own_signature_fails_adopts_no_link() {
    let (_, ka) = keyed(140);
    let (b_log, kb) = keyed(150);
    let pins = AllowedSigners::new()
        .allow(ka.identity().clone(), BUNDLE_KINDS)
        .allow(kb.identity().clone(), BUNDLE_KINDS);
    let good = linked_bundle(
        &b_log,
        &kb,
        Vec::new(),
        pins.clone(),
        vec![rotation_link(&ka, &kb)],
    );
    let other = forged_bundle(&b_log, &kb, Vec::new(), AllowedSigners::new());
    let (good, other) = (
        decode_signed(&good).expect("bundle"),
        decode_signed(&other).expect("bundle"),
    );
    let spliced = encode_signed(good.body_bytes(), other.signature());
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(pins),
        ..Setup::default()
    });
    let (outcome, adopted, entered) = import_outcome(&importer.import(spliced));
    assert_eq!(outcome, SignatureOutcome::SignatureMismatch);
    assert_eq!((adopted, entered), (0, 0));
    assert!(standing(&importer, ka.identity()).is_none());
    assert!(
        importer
            .daemon
            .state()
            .signing()
            .registry()
            .audit_log()
            .is_empty()
    );
}

#[test]
fn an_import_refused_for_quota_adopts_nothing() {
    let (_, k1) = keyed(150);
    let (k2_log, k2) = keyed(151);
    let actor = SigningActor::new("human:peer").expect("actor");
    let pins = AllowedSigners::new()
        .allow(k1.identity().clone(), BUNDLE_KINDS)
        .allow(k2.identity().clone(), BUNDLE_KINDS);
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(pins.clone()),
        ..Setup::default()
    });
    let rotation = rotation_link(&k1, &k2);
    for n in 0..continuumd::daemon::signing::MAX_HELD_BUNDLES {
        let mut seed = [0u8; SEED_LEN];
        seed[..8].copy_from_slice(&(n as u64).to_le_bytes());
        seed[31] = 0x5A;
        let mut scratch = SigningRegistry::new();
        let other = scratch
            .mint(&actor, &mut OneSeed(seed))
            .expect("mint")
            .identity()
            .clone();
        let allowed = pins.clone().allow(other, [Kind::Receipt]);
        let bytes = linked_bundle(&k2_log, &k2, Vec::new(), allowed, vec![rotation.clone()]);
        let (outcome, _, _) = import_outcome(&importer.import(bytes));
        assert_eq!(outcome, SignatureOutcome::Verified, "bundle {n}");
    }
    let before = importer.daemon.state().signing().registry().clone();
    let bytes = linked_bundle(
        &k2_log,
        &k2,
        Vec::new(),
        pins,
        vec![rotation, revocation_link(&k1)],
    );
    let refused = importer.import(bytes);
    assert_eq!(refused.error_code(), Some(ErrorCode::QuotaExhausted));
    assert_eq!(
        importer.daemon.state().signing().registry(),
        &before,
        "a refused import adopts nothing"
    );
}

#[test]
fn a_held_key_revocation_always_leaves_room_for_its_replacement() {
    let max = continuumd::daemon::bundle::MAX_AUDIT_RECORDS;
    let actor = SigningActor::new("human:solo-dev").expect("actor");
    // One pool of other signers, derived once and shared by every length.
    let others: Vec<SignerIdentity> = (1..=4095u32)
        .map(|n| {
            let mut seed = [0u8; SEED_LEN];
            seed[..4].copy_from_slice(&n.to_le_bytes());
            seed[31] = 0xA5;
            SigningRegistry::new()
                .mint(&actor, &mut OneSeed(seed))
                .expect("mint")
                .identity()
                .clone()
        })
        .collect();
    let at = |records: usize| {
        let mut registry = SigningRegistry::new();
        let key = registry.mint(&actor, &mut Seeds(1)).expect("mint");
        for other in &others[..records - 1] {
            registry
                .record_observed(
                    &actor,
                    continuum_evidence::signing::SigningEvent::Minted {
                        signer: other.clone(),
                    },
                )
                .expect("a new signer");
        }
        world(Setup {
            entropy: Some(200),
            signer: Some(ReceiptSigner::new(registry, key).expect("installable")),
            ..Setup::default()
        })
    };
    // Every installable length near the bound: 4093 is the last (install needs the
    // recovery reserve, `an_install_that_would_leave_a_compromise_unrecoverable_is_refused`).
    for records in [4030, 4031, 4032, 4092, 4093] {
        // A loss of the held key: recovered with a successor while that successor could
        // in turn be revoked, and otherwise revoked plainly. Either way it is retired.
        let mut lost = at(records);
        let held = lost.daemon.state().signing().held().cloned().expect("held");
        let recovered = lost.call(
            STEWARD,
            Arguments::SigningRevoke(SigningRevokeRequest {
                signer: signer_handle(&held),
                reason: RevocationReason::KeyLost,
            }),
        );
        let Payload::SigningRevoke(recovery) = recovered.payload else {
            panic!("a loss at {records}: {:?}", recovered.error_code());
        };
        assert_eq!(
            matches!(recovery.successor, Nullable::Value(_)),
            records + 4 <= max,
            "at {records}"
        );
        assert!(is_revoked(standing(&lost, &held)));
        if !matches!(recovery.successor, Nullable::Value(_)) {
            // A plain revocation: the replacement mint still has its room.
            let replaced = lost.call(
                STEWARD,
                Arguments::SigningMint(SigningMintRequest {
                    kinds: vec![SignedArtifactKind::Receipt],
                }),
            );
            assert_eq!(
                replaced.error_code(),
                None,
                "a replacement after a loss at {records}"
            );
        }

        let mut world = at(records);
        let held = world
            .daemon
            .state()
            .signing()
            .held()
            .cloned()
            .expect("held");
        // An active held key can always be revoked.
        let revoked = world.call(
            STEWARD,
            Arguments::SigningRevoke(SigningRevokeRequest {
                signer: signer_handle(&held),
                reason: RevocationReason::Compromised,
            }),
        );
        assert_eq!(revoked.error_code(), None, "a revocation at {records}");
        let replaced = world.call(
            STEWARD,
            Arguments::SigningMint(SigningMintRequest {
                kinds: vec![SignedArtifactKind::Receipt],
            }),
        );
        // Every installable registry can revoke its held key and replace it, and the
        // replacement can in turn be revoked.
        let Payload::SigningMint(minted) = replaced.payload else {
            panic!("a replacement at {records}: {:?}", replaced.error_code());
        };
        let again = world.call(
            STEWARD,
            Arguments::SigningRevoke(SigningRevokeRequest {
                signer: minted.signer,
                reason: RevocationReason::Compromised,
            }),
        );
        assert_eq!(
            again.error_code(),
            None,
            "the replacement is revocable at {records}"
        );
        assert!(world.records() <= max);
    }
}

/// A registry where each of `keys` is active.
fn knowing(keys: &[&continuum_evidence::signing::LocalSigner]) -> SigningRegistry {
    let actor = SigningActor::new("human:peer").expect("actor");
    let mut registry = SigningRegistry::new();
    for key in keys {
        registry
            .record_observed(
                &actor,
                continuum_evidence::signing::SigningEvent::Minted {
                    signer: key.identity().clone(),
                },
            )
            .expect("a new signer");
    }
    registry
}

/// The rotation `from → to`, attested by both keys.
fn rotation_link(
    from: &continuum_evidence::signing::LocalSigner,
    to: &continuum_evidence::signing::LocalSigner,
) -> SignerLink {
    knowing(&[from, to])
        .attest_rotation(from, to)
        .expect("both active")
}

/// `key`'s own compromise revocation.
fn revocation_link(key: &continuum_evidence::signing::LocalSigner) -> SignerLink {
    knowing(&[key]).attest_revocation(key).expect("active")
}

// --- the acceptance chain (RFC 0037 A1–A4, cr-2unxyh round 3) -------------------------------

/// A registry record for `intent` as `rule intent.bundles` carries one, with the given
/// acceptance `signature` and `chain` (each element `(signer, signature)`).
fn record_json(
    intent: &IntentHandle,
    supersedes: Option<&IntentHandle>,
    signature: &str,
    chain: &[(String, String)],
) -> Vec<u8> {
    let text = |value: &str| ContractJson::String(value.to_owned());
    let acceptance = BTreeMap::from([
        ("accepted_by".to_owned(), text(STEWARD.0)),
        ("audit_record".to_owned(), text("a1")),
        ("capability".to_owned(), text("revise-intent")),
        ("signature".to_owned(), text(signature)),
        ("timestamp".to_owned(), text(ACCEPTED_AT)),
    ]);
    let mut fields = BTreeMap::from([
        (
            "schema_id".to_owned(),
            text("https://continuum.dev/schema/intent-registry-record.json"),
        ),
        ("schema_epoch".to_owned(), ContractJson::Integer(1)),
        ("record_id".to_owned(), text(intent.as_str())),
        ("intent".to_owned(), text(intent.as_str())),
        ("status".to_owned(), text("accepted")),
        (
            "supersedes".to_owned(),
            supersedes.map_or(ContractJson::Null, |base| text(base.as_str())),
        ),
        ("superseded_by".to_owned(), ContractJson::Null),
        ("acceptance".to_owned(), ContractJson::Object(acceptance)),
    ]);
    if !chain.is_empty() {
        fields.insert(
            "chain".to_owned(),
            ContractJson::Array(
                chain
                    .iter()
                    .map(|(signer, signature)| {
                        ContractJson::Object(BTreeMap::from([
                            ("scope".to_owned(), text("revise-intent")),
                            ("signature".to_owned(), text(signature)),
                            ("signer".to_owned(), text(signer)),
                        ]))
                    })
                    .collect(),
            ),
        );
    }
    ContractJson::Object(fields).to_canonical_bytes()
}

/// A chain element `key` signs over the A1 statement for `intent` accepted by `by`.
fn acceptance_element(
    registry: &SigningRegistry,
    key: &continuum_evidence::signing::LocalSigner,
    intent: &IntentHandle,
    by: &str,
    base: Option<&IntentHandle>,
) -> (String, String) {
    use continuumd::daemon::acceptance::{Statement, element};
    let statement = Statement {
        intent,
        base,
        accepted_by: by,
        timestamp: ACCEPTED_AT,
    };
    let signature = registry
        .sign(key, Kind::IntentAcceptance, &statement.identity(&[]))
        .expect("active");
    let element = element(&signature);
    (element.signer, element.signature)
}

/// A bundle signer cannot turn a record into an acceptance its chain does not sign. The
/// chain's key vouches for its own daemon's admission of `accepted_by` at `timestamp` (the
/// daemon signs only the principal it admitted, at its own time); a verifier binds the
/// record, the local lineage, and the presented acceptance to exactly what the key signed.
#[test]
fn a_record_is_accepted_only_as_its_chain_signed_it() {
    let (k_log, k) = keyed(160);
    let proposal_contract = contract(None);
    let proposal = intent_handle(&proposal_contract);
    let entry = |record: Vec<u8>| BundleContract {
        intent: proposal.clone(),
        contract: proposal_contract.to_artifact_bytes(),
        record,
    };
    let honest = acceptance_element(&k_log, &k, &proposal, STEWARD.0, None);
    let cases: Vec<(&str, Vec<u8>, bool)> = vec![
        (
            "the chain an accepting key really signed",
            record_json(&proposal, None, &honest.1, std::slice::from_ref(&honest)),
            true,
        ),
        (
            "no chain: a bare string where the signature goes",
            record_json(&proposal, None, &"ab".repeat(64), &[]),
            false,
        ),
        (
            "a chain over another principal",
            {
                let other = acceptance_element(&k_log, &k, &proposal, "human:mallory", None);
                record_json(&proposal, None, &other.1, std::slice::from_ref(&other))
            },
            false,
        ),
        (
            "a chain over another base, on a genesis record (A2)",
            {
                let base = IntentHandle::new("in_base").expect("an in_");
                let other = acceptance_element(&k_log, &k, &proposal, STEWARD.0, Some(&base));
                record_json(&proposal, None, &other.1, std::slice::from_ref(&other))
            },
            false,
        ),
        (
            "the acceptance signature is not the chain's last element",
            {
                record_json(
                    &proposal,
                    None,
                    &"00".repeat(40),
                    std::slice::from_ref(&honest),
                )
            },
            false,
        ),
    ];
    for (case, record, accepts) in cases {
        for kinds in [BUNDLE_KINDS.to_vec(), vec![Kind::IntentBundle]] {
            let pins = AllowedSigners::new().allow(k.identity().clone(), kinds.clone());
            let mut importer = world(Setup {
                entropy: Some(100),
                allowed: Some(pins.clone()),
                ..Setup::default()
            });
            let bytes = forged_bundle(&k_log, &k, vec![entry(record.clone())], pins);
            let (outcome, _, entered) = import_outcome(&importer.import(bytes.clone()));
            assert_eq!(
                (outcome, entered),
                (SignatureOutcome::Verified, 1),
                "{case}: every case imports, so each refusal is the chain's"
            );
            let bundle = bundle_handle_of(&importer, &bytes);
            let outcome = importer.accept(&proposal, Some(&bundle), FROM_BUNDLE);
            let allowed = kinds.contains(&Kind::IntentAcceptance);
            assert_eq!(
                outcome.error_code(),
                if accepts && allowed {
                    None
                } else {
                    Some(ErrorCode::AcceptanceChainInvalid)
                },
                "{case}, acceptance {}allowed by local policy",
                if allowed { "" } else { "not " }
            );
        }
    }
}

fn bundle_handle_of(world: &World, content: &[u8]) -> IntentBundleHandle {
    let _ = world;
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        ArtifactClass::SignedIntentBundle,
        content,
    )
    .expect("an identity");
    continuumd::daemon::identity::bundle_to_wire(&stored).expect("an inb_")
}

#[test]
fn an_acceptance_by_a_rotated_or_revoked_signer_does_not_vouch() {
    // The exporter accepts P with key A, rotates to B, and exports: B's bundle is fine, but
    // P's acceptance is A's, and A is retired — so the stricter reading refuses it.
    let mut exporter = with_entropy(10);
    let (a_name, a) = exporter.mint(&[SignedArtifactKind::IntentBundle]);
    let proposal = exporter.put_accepted(&contract(None), None);
    let rotated = exporter.call(
        STEWARD,
        Arguments::SigningRotate(SigningRotateRequest { signer: a_name }),
    );
    let Payload::SigningRotate(rotation) = rotated.payload else {
        panic!("rotate failed");
    };
    let b = SignerIdentity::from_public_key(&rotation.public_key).expect("a key");
    let (bundle, content) = exporter.export(vec![proposal.clone()]);
    let pins = AllowedSigners::new()
        .allow(a, BUNDLE_KINDS)
        .allow(b, BUNDLE_KINDS);
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(pins),
        ..Setup::default()
    });
    let (outcome, _, entered) = import_outcome(&importer.import(content));
    assert_eq!((outcome, entered), (SignatureOutcome::Verified, 1));
    assert_eq!(
        importer
            .accept(&proposal, Some(&bundle), FROM_BUNDLE)
            .error_code(),
        Some(ErrorCode::AcceptanceChainInvalid)
    );
    // (Stricter reading: with no trusted time, a rotated key's acceptance cannot be dated
    // before its rotation, so it does not vouch; a successor countersignature is a
    // follow-up.) A key revoked locally before the acceptance does not vouch either.
    let mut fresh = with_entropy(10);
    let (_, key) = fresh.mint(&[SignedArtifactKind::IntentBundle]);
    let proposal = fresh.put_accepted(&contract(Some(5)), None);
    let (bundle, content) = fresh.export(vec![proposal.clone()]);
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(AllowedSigners::new().allow(key.clone(), BUNDLE_KINDS)),
        ..Setup::default()
    });
    let _ = importer.import(content);
    let revoked = importer.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: signer_handle(&key),
            reason: RevocationReason::Compromised,
        }),
    );
    assert_eq!(revoked.error_code(), None);
    assert_eq!(
        importer
            .accept(&proposal, Some(&bundle), FROM_BUNDLE)
            .error_code(),
        Some(ErrorCode::AcceptanceChainInvalid)
    );
}

#[test]
fn a_local_acceptance_at_3_8_records_a_verifiable_chain() {
    let mut exporter = with_entropy(10);
    let (_, key) = exporter.mint(&[SignedArtifactKind::IntentBundle]);
    let proposal = exporter.put_accepted(&contract(None), None);
    let record = exporter.daemon.state().intent(&proposal).expect("held");
    let acceptance = record.acceptance.as_ref().expect("accepted");
    assert_eq!(acceptance.chain.len(), 1);
    assert_eq!(acceptance.chain[0].signer, key.handle().as_str());
    assert_eq!(acceptance.signature, acceptance.chain[0].signature);
    assert_ne!(
        acceptance.signature, CALLER_TEXT,
        "the caller's text is replaced"
    );
    // Verifies under the exporter's own registry and policy.
    let statement = continuumd::daemon::acceptance::Statement {
        intent: &proposal,
        base: None,
        accepted_by: STEWARD.0,
        timestamp: ACCEPTED_AT,
    };
    let signing = exporter.daemon.state().signing();
    assert_eq!(
        continuumd::daemon::signing::verify_acceptance_chain(
            signing.allowed(),
            signing.registry(),
            &signing.registry().head(),
            &statement,
            &acceptance.chain,
            &acceptance.signature,
        ),
        Ok(())
    );
    // A daemon with no key records the caller's text and no chain.
    let mut keyless = with_entropy(10);
    let unsigned = keyless.put_accepted(&contract(Some(6)), None);
    let held = keyless.daemon.state().intent(&unsigned).expect("held");
    let acceptance = held.acceptance.as_ref().expect("accepted");
    assert!(acceptance.chain.is_empty());
    assert_eq!(acceptance.signature, CALLER_TEXT);
}

// --- multi-hop rotation (cr-2unxyh round 3) --------------------------------------------------

fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut out = Vec::new();
    for (at, item) in items.iter().enumerate() {
        let mut rest = items.to_vec();
        rest.remove(at);
        for mut tail in permutations(&rest) {
            tail.insert(0, item.clone());
            out.push(tail);
        }
    }
    out
}

#[test]
fn every_order_of_a_rotation_chain_gives_the_same_standing_and_facts() {
    use continuum_evidence::signing::SignerStanding;
    let keys: Vec<_> = (0..4u8).map(|n| keyed(170 + n)).collect();
    let pins = keys.iter().fold(AllowedSigners::new(), |set, (_, key)| {
        set.allow(key.identity().clone(), BUNDLE_KINDS)
    });
    for hops in [2usize, 3] {
        let links: Vec<SignerLink> = (0..hops)
            .map(|at| rotation_link(&keys[at].1, &keys[at + 1].1))
            .chain(std::iter::once(revocation_link(&keys[0].1)))
            .collect();
        let (last_log, last) = &keys[hops];
        let mut results = Vec::new();
        for order in permutations(&links) {
            // The importer already knows A, the chain's first key.
            let mut importer = world(Setup {
                entropy: Some(100),
                allowed: Some(pins.clone()),
                ..Setup::default()
            });
            let (a_log, a) = &keys[0];
            let bytes = forged_bundle(a_log, a, Vec::new(), pins.clone());
            assert_eq!(import_outcome(&importer.import(bytes)).1, 1);
            let bytes = linked_bundle(last_log, last, Vec::new(), pins.clone(), order);
            let (outcome, _, _) = import_outcome(&importer.import(bytes));
            assert_eq!(outcome, SignatureOutcome::Verified);
            let registry = importer.daemon.state().signing().registry();
            let standings: Vec<_> = keys[..=hops]
                .iter()
                .map(|(_, key)| registry.standing(key.identity()).cloned())
                .collect();
            let facts: Vec<_> = registry
                .audit_log()
                .iter()
                .map(|record| record.event().clone())
                .collect();
            results.push((standings, facts));
        }
        let first = results[0].clone();
        assert!(results.iter().all(|result| *result == first), "{hops} hops");
        // A ends revoked (it was rotated first), every middle key rotated, the last active.
        assert!(matches!(first.0[0], Some(SignerStanding::Revoked { .. })));
        for middle in &first.0[1..hops] {
            assert!(matches!(middle, Some(SignerStanding::Rotated { .. })));
        }
        assert_eq!(first.0[hops], Some(SignerStanding::Active));
        assert!(first.1.iter().any(|fact| matches!(
            fact,
            continuum_evidence::signing::SigningEvent::Rotated { from, .. } if from == keys[0].1.identity()
        )));
    }
}

#[test]
fn a_prelearned_first_key_ends_rotated_and_fails_ci_acceptance() {
    // The exporter accepts P under A, exports, then rotates A -> B -> C and exports from C
    // carrying both links. The importer knew A. Every carried order is covered by
    // `every_order_of_a_rotation_chain_gives_the_same_standing_and_facts`.
    let mut exporter = with_entropy(10);
    let (mut name, a) = exporter.mint(&[SignedArtifactKind::IntentBundle]);
    let proposal = exporter.put_accepted(&contract(None), None);
    let (a_bundle, a_content) = exporter.export(vec![proposal.clone()]);
    let mut keys = vec![a];
    for _ in 0..2 {
        let rotated = exporter.call(
            STEWARD,
            Arguments::SigningRotate(SigningRotateRequest { signer: name }),
        );
        let Payload::SigningRotate(rotation) = rotated.payload else {
            panic!("rotate failed");
        };
        keys.push(SignerIdentity::from_public_key(&rotation.public_key).expect("a key"));
        name = rotation.successor;
    }
    let other = exporter.put_accepted(&contract(Some(8)), None);
    let (_, c_content) = exporter.export(vec![other]);
    let pins = keys.iter().fold(AllowedSigners::new(), |set, key| {
        set.allow(key.clone(), BUNDLE_KINDS)
    });
    let mut importer = world(Setup {
        entropy: Some(100),
        allowed: Some(pins),
        ..Setup::default()
    });
    let _ = importer.import(a_content);
    assert_eq!(
        standing(&importer, &keys[0]),
        Some(continuum_evidence::signing::SignerStanding::Active)
    );
    let (outcome, _, _) = import_outcome(&importer.import(c_content));
    assert_eq!(outcome, SignatureOutcome::Verified);
    assert!(matches!(
        standing(&importer, &keys[0]),
        Some(continuum_evidence::signing::SignerStanding::Rotated { .. })
    ));
    assert_eq!(
        importer
            .accept(&proposal, Some(&a_bundle), FROM_BUNDLE)
            .error_code(),
        Some(ErrorCode::AcceptanceChainInvalid),
        "A's bundle no longer passes CI acceptance"
    );
}

#[test]
fn a_cycle_or_a_shared_successor_is_refused_whole() {
    // Every malformed topology, in every carried order, is refused as malformed with
    // nothing changed; none of them may hang the import.
    let keys: Vec<_> = (0..4u8).map(|n| keyed(180 + n)).collect();
    let (a, b, c, d) = (&keys[0].1, &keys[1].1, &keys[2].1, &keys[3].1);
    let pins = keys.iter().fold(AllowedSigners::new(), |set, (_, key)| {
        set.allow(key.identity().clone(), BUNDLE_KINDS)
    });
    let (b_log, _) = &keys[1];
    let cases: Vec<(&str, Vec<SignerLink>)> = vec![
        (
            "a pure 2-cycle",
            vec![rotation_link(a, b), rotation_link(b, a)],
        ),
        (
            "a pure 3-cycle",
            vec![
                rotation_link(a, b),
                rotation_link(b, c),
                rotation_link(c, a),
            ],
        ),
        (
            "a tail into a 2-cycle",
            vec![
                rotation_link(a, b),
                rotation_link(b, c),
                rotation_link(c, b),
            ],
        ),
        (
            "a tail into a 3-cycle",
            vec![
                rotation_link(a, b),
                rotation_link(b, c),
                rotation_link(c, d),
                rotation_link(d, b),
            ],
        ),
        (
            "a shared successor",
            vec![rotation_link(a, c), rotation_link(b, c)],
        ),
        (
            "a key rotated twice",
            vec![rotation_link(a, b), rotation_link(a, c)],
        ),
        (
            "a replayed rotation",
            vec![rotation_link(a, b), rotation_link(a, b)],
        ),
        (
            "a replayed revocation",
            vec![revocation_link(a), revocation_link(a)],
        ),
    ];
    for (case, links) in cases {
        for order in permutations(&links) {
            assert!(
                !continuumd::daemon::signing::links_are_attested(&order),
                "{case}"
            );
            let mut importer = world(Setup {
                entropy: Some(100),
                allowed: Some(pins.clone()),
                ..Setup::default()
            });
            let bytes = linked_bundle(b_log, b, Vec::new(), pins.clone(), order);
            assert_eq!(
                importer.import(bytes).error_code(),
                Some(ErrorCode::MalformedRequest),
                "{case}"
            );
            let signing = importer.daemon.state().signing();
            assert!(signing.registry().audit_log().is_empty(), "{case}");
            assert_eq!(signing.unattested_links(), 1, "{case}");
        }
    }
}

// --- receipts: exact bytes, never re-signed (cr-2unxyh round 3) -----------------------------

#[test]
fn a_receipt_node_keeps_its_first_bytes_and_its_first_signature() {
    let mut world = with_entropy(30);
    let staging = Colliding {
        intents: false,
        bundles: false,
        receipts: false,
    };
    let stage = |daemon: &mut Daemon, path: &str, content: &[u8]| {
        daemon
            .state_mut()
            .stage(
                &staging,
                WorkspacePath::new(path).expect("path"),
                content.to_vec(),
            )
            .expect("staged")
    };
    let trace = stage(&mut world.daemon, "traces/t.jsonl", b"{\"events\":[]}\n");
    let bytes = b"{\"receipt\":\"checked\"}\n".to_vec();
    let receipt = stage(&mut world.daemon, "receipts/r.json", &bytes);
    let ingested = world.call(
        PRODUCER,
        Arguments::ObserveIngest(ObserveIngestRequest {
            trace,
            instrumentation_profile: "otel-1.0/sampled".to_owned(),
        }),
    );
    let Payload::ObserveIngest(ingest) = ingested.payload else {
        panic!("ingest failed");
    };
    let link = |world: &mut World| {
        world.call(
            CHECKER,
            Arguments::EvidenceLink(EvidenceLinkRequest {
                subject: ingest.evidence[0].clone(),
                receipt: receipt.clone(),
                checker_profile: "kernel-core/1".to_owned(),
            }),
        )
    };
    let signature_of = |world: &mut World, node: &continuumd::protocol::scalar::EvidenceHandle| {
        let got = world.call(
            READER,
            Arguments::EvidenceGet(EvidenceGetRequest {
                evidence: node.clone(),
                inline: Optional::Absent,
            }),
        );
        let Payload::EvidenceGet(node) = got.payload else {
            panic!("evidence.get failed");
        };
        node.signature
    };
    // Linked with no key: unsigned.
    let first = link(&mut world);
    let Payload::EvidenceLink(first) = first.payload else {
        panic!("link failed");
    };
    assert_eq!(signature_of(&mut world, &first.receipt), Optional::Absent);
    // A key appears; the same bytes relinked are signed, once.
    let (name, key) = world.mint(&[SignedArtifactKind::Receipt]);
    assert_eq!(link(&mut world).error_code(), None);
    let Optional::Present(signed) = signature_of(&mut world, &first.receipt) else {
        panic!("signed on relink over identical bytes");
    };
    assert_eq!(
        ArtifactSignature::decode(&signed).expect("record").signer(),
        &key
    );
    assert_eq!(
        world.verify(SignedArtifactKind::Receipt, &bytes, Some(signed.clone())),
        SignatureOutcome::Verified
    );
    // After a rotation, a relink does not replace the signature.
    let rotated = world.call(
        STEWARD,
        Arguments::SigningRotate(SigningRotateRequest { signer: name }),
    );
    assert_eq!(rotated.error_code(), None);
    assert_eq!(link(&mut world).error_code(), None);
    assert_eq!(
        signature_of(&mut world, &first.receipt),
        Optional::Present(signed.clone())
    );
    // A revoked key refuses the relink, and the node keeps its signature.
    let held = world
        .daemon
        .state()
        .signing()
        .held()
        .cloned()
        .expect("held");
    let _ = world.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: signer_handle(&held),
            reason: RevocationReason::Compromised,
        }),
    );
    assert!(link(&mut world).error_code().is_some());
    assert_eq!(
        signature_of(&mut world, &first.receipt),
        Optional::Present(signed)
    );
}

/// An identity seam that names every staged record alike.
#[derive(Debug, Clone, Copy)]
struct CollidingStage;

impl continuum_workspace::publication::ContentIdentifier for CollidingStage {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<
        continuum_workspace::artifact_path::ArtifactHandle,
        continuum_workspace::publication::IdentityUnavailable,
    > {
        let bytes: &[u8] = if class == ArtifactClass::WorkspaceSnapshot {
            b"one commitment for every record"
        } else {
            content
        };
        continuum_workspace::publication::ContentIdentifier::identify(&Blake3Identity, class, bytes)
    }
}

#[test]
fn staging_never_overwrites_content_under_a_colliding_commitment() {
    let mut world = with_entropy(30);
    let path = WorkspacePath::new("receipts/r.json").expect("path");
    let first = world
        .daemon
        .state_mut()
        .stage(&CollidingStage, path.clone(), b"first".to_vec())
        .expect("staged");
    let again = world
        .daemon
        .state_mut()
        .stage(&CollidingStage, path.clone(), b"first".to_vec())
        .expect("the same record again is idempotent");
    assert_eq!(first, again);
    assert_eq!(
        world
            .daemon
            .state_mut()
            .stage(&CollidingStage, path, b"second".to_vec()),
        Err(continuumd::daemon::ServiceError::Collision)
    );
    assert_eq!(
        world.daemon.state().staged(&first).expect("held").content,
        b"first".to_vec()
    );
}

// --- install (cr-2unxyh round 3) ------------------------------------------------------------

#[test]
fn an_install_that_would_leave_a_compromise_unrecoverable_is_refused() {
    let max = continuumd::daemon::bundle::MAX_AUDIT_RECORDS;
    let actor = SigningActor::new("human:solo-dev").expect("actor");
    let others: Vec<SignerIdentity> = (1..=max as u32)
        .map(|n| {
            let mut seed = [0u8; SEED_LEN];
            seed[..4].copy_from_slice(&n.to_le_bytes());
            seed[31] = 0x5C;
            SigningRegistry::new()
                .mint(&actor, &mut OneSeed(seed))
                .expect("mint")
                .identity()
                .clone()
        })
        .collect();
    for records in [4092, 4093, 4094, 4095, 4096, 4097] {
        let mut registry = SigningRegistry::new();
        let key = registry.mint(&actor, &mut Seeds(1)).expect("mint");
        for other in &others[..records - 1] {
            registry
                .record_observed(
                    &actor,
                    continuum_evidence::signing::SigningEvent::Minted {
                        signer: other.clone(),
                    },
                )
                .expect("a new signer");
        }
        let installed = ReceiptSigner::new(registry, key);
        // Installable exactly when revocation, a replacement mint, and the replacement's
        // revocation all fit.
        match records {
            4092 | 4093 => assert!(installed.is_ok(), "at {records}"),
            4094..=4096 => assert_eq!(
                installed.err(),
                Some(continuumd::daemon::InstallRefusal::NoRoomToRecover),
                "at {records}"
            ),
            _ => assert_eq!(
                installed.err(),
                Some(continuumd::daemon::InstallRefusal::RecordBound)
            ),
        }
    }
}

#[test]
fn a_signed_local_acceptance_names_only_the_admitted_principal_at_the_daemons_time() {
    let mut world = with_entropy(10);
    let _ = world.mint(&[SignedArtifactKind::IntentBundle]);
    let proposal = intent_handle(&contract(None));
    world.daemon.state_mut().put_intent(
        proposal.clone(),
        IntentRecord {
            contract: contract(None),
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );
    let accept_as = |world: &mut World, by: &str, at: &str| {
        world.call(
            STEWARD,
            Arguments::IntentAccept(IntentAcceptRequest {
                proposal: proposal.clone(),
                acceptance: acceptance(by, CALLER_TEXT, at),
                bundle: Optional::Absent,
            }),
        )
    };
    for (by, at) in [
        ("human:ceo", ACCEPTED_AT),
        (STEWARD.0, "2020-01-01T00:00:00.000Z"),
    ] {
        assert_eq!(
            accept_as(&mut world, by, at).error_code(),
            Some(ErrorCode::AcceptanceChainInvalid),
            "{by} at {at}"
        );
        assert_eq!(
            world.daemon.state().intent(&proposal).expect("held").status,
            RegistryStatus::Proposed
        );
    }
    // A revoked acceptance key refuses rather than record the acceptance unsigned.
    let held = world
        .daemon
        .state()
        .signing()
        .held()
        .cloned()
        .expect("held");
    let _ = world.call(
        STEWARD,
        Arguments::SigningRevoke(SigningRevokeRequest {
            signer: signer_handle(&held),
            reason: RevocationReason::Compromised,
        }),
    );
    assert_eq!(
        accept_as(&mut world, STEWARD.0, ACCEPTED_AT).error_code(),
        Some(ErrorCode::UnsupportedSemanticFeature)
    );
    assert_eq!(
        world.daemon.state().intent(&proposal).expect("held").status,
        RegistryStatus::Proposed
    );
}
