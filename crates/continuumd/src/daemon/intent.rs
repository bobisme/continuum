//! The `intent` family: `get`, `diff`, `propose_revision`, `accept`, `reject`, `lock`.
//!
//! # Where the privileged boundary actually is
//!
//! All six verbs sit at `authority revise-intent` or below, and three of them —
//! `accept`, `reject`, `lock` — are additionally `@privileged @audit_recorded`. RFC 0027 is
//! explicit that the difference between proposing and accepting is *not* a level:
//!
//! > `intent.propose_revision` is **not** `@privileged`, so a capability at `revise-intent`
//! > without privileged membership can propose a revision and cannot accept one — which is
//! > precisely docs/49's "revise intent: proposal only" cell, and it is unreachable if
//! > privilege is read as a level.
//! >
//! > — RFC 0027, "The admission predicate"
//!
//! So this module contains **no authority code at all**. T3 is decided in
//! [`admission`](super::admission) from `CapabilityDescriptor.profile.privileged_operations`
//! and the registry's `@privileged` annotation, before any handler runs; every refusal it
//! produces is the single indistinguishable `CapabilityDenied` (X1), and every one of them
//! is audited whether or not it succeeded (P5). A reader looking for "who may accept an
//! intent" will not find an answer here, and that is the design.
//!
//! # `IntentMutationDenied` is a different question from `CapabilityDenied`
//!
//! The IDL defines it as "A protected Intent Contract field was mutated without the
//! `revise-intent` capability, **or against an intent lock** (plan §5.4)". The first clause
//! is admission's and never reaches a handler. The second is this module's, and it is
//! decided against RFC 0037's landed field-level change policy:
//!
//! - `intent.lock` refuses an edit that *weakens* a field's verb — the verb lattice's own
//!   order, [`PolicyVerb::is_weaker_or_equal`], decides — because a lock a privileged call
//!   can lift is not a lock;
//! - `intent.propose_revision` refuses a change set naming a field whose verb is `locked`,
//!   because `locked` "contributes `block` for any non-`unchanged` relation" (RFC 0037 P4)
//!   and a change set naming a field is not proposing that it is unchanged.
//!
//! Neither refusal is reachable without first passing T1–T4, so the two codes answer two
//! different questions and never stand in for one another.
//!
//! # What is refused rather than approximated
//!
//! `intent.diff` and `intent.propose_revision` both have to return a `diff_*` handle naming
//! an RFC 0031 artifact, and `intent.diff` additionally has to return that artifact inline.
//! Classification — deciding *which* of RFC 0031's sixteen relations a change is — is
//! `crates/continuum-semantic-diff`'s, which has not shipped, and R2 permits `unchanged` on
//! canonical equality and nothing weaker. Both therefore return
//! [`ErrorCode::UnsupportedSemanticFeature`], the code both `errors` clauses declare, rather
//! than guessing a relation: a weakening reported as `unchanged` is the first attack
//! docs/50 lists.
//!
//! # Identity, and what a governance edit moves
//!
//! `intent.accept` records acceptance and does **not** mint a new identity:
//!
//! > Acceptance and supersession are not in the `in_*` identity preimage (RFC 0037 ID4), so
//! > recording them here changes no contract identity.
//! >
//! > — `schemas/intent-registry-record.schema.json`, `superseded_by`
//!
//! `intent.lock` does mint one, because the policy table *is* in the preimage (ID2) — "which
//! is why ID3 records that `intent.lock` mints a *successor* contract" — so the response's
//! `intent` is the successor's handle, the predecessor's record becomes `superseded`, and
//! the two records name each other. The `intent_id` field takes no part in the identity
//! (ID2 excludes it), which is what lets the successor be named before it is stamped with
//! its own name.
//!
//! [`PolicyVerb::is_weaker_or_equal`]: continuum_intent::change_policy::PolicyVerb::is_weaker_or_equal

use std::collections::BTreeMap;

use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{PolicyField, PolicyTable, PolicyVerb};
use continuum_intent::contract::{ContractParts, IntentContract, IntentId};
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::ReferenceStore;

use super::family::{Arguments, Call, Effect, Fault, OperationFamily, Payload, ScopeClaim};
use super::state::{Acceptance, DaemonState, IntentRecord, RegistryStatus};
use super::{Services, identity};
use crate::protocol::envelope::{StructuralVerdictValue, Verdict};
use crate::protocol::operations::intent::{
    IntentAcceptRequest, IntentAcceptResponse, IntentGetRequest, IntentGetResponse,
    IntentLockRequest, IntentLockResponse, IntentProposeRevisionRequest, IntentRejectRequest,
    IntentRejectResponse,
};
use crate::protocol::scalar::{IntentHandle, Opaque, Timestamp};
use crate::protocol::spec::Nullable;
use crate::protocol::vocabulary::{ErrorCode, StructuralOutcome};

/// The `intent` namespace's six operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct IntentFamily;

/// Every `(operation, code)` pair this family can answer with.
///
/// Held to `rule errors.common` ∪ each operation's `errors` clause by
/// `tests/daemon_operations.rs`.
pub const FAULTS: &[(&str, ErrorCode)] = &[
    ("intent.get", ErrorCode::CapabilityDenied),
    ("intent.diff", ErrorCode::UnsupportedSemanticFeature),
    (
        "intent.propose_revision",
        ErrorCode::UnsupportedSemanticFeature,
    ),
    ("intent.propose_revision", ErrorCode::IntentMutationDenied),
    ("intent.propose_revision", ErrorCode::CapabilityDenied),
    ("intent.propose_revision", ErrorCode::MalformedRequest),
    ("intent.accept", ErrorCode::CapabilityDenied),
    ("intent.accept", ErrorCode::AcceptanceChainInvalid),
    ("intent.accept", ErrorCode::IntentMutationDenied),
    ("intent.reject", ErrorCode::CapabilityDenied),
    ("intent.reject", ErrorCode::IntentMutationDenied),
    ("intent.lock", ErrorCode::CapabilityDenied),
    ("intent.lock", ErrorCode::IntentMutationDenied),
    ("intent.lock", ErrorCode::MalformedRequest),
];

/// The `$id` of the governing schema, with the `v<schema_epoch>/` segment removed
/// (`schemas/README.md`). Stable across schema epochs.
const RECORD_SCHEMA_ID: &str = "https://continuum.dev/schema/intent-registry-record.json";

/// The schema epoch this daemon writes registry records at.
const RECORD_SCHEMA_EPOCH: i64 = 1;

/// The one value `intent-registry-record.schema.json` admits for `acceptance.capability`.
const ACCEPTANCE_CAPABILITY: &str = "revise-intent";

impl OperationFamily for IntentFamily {
    fn namespace(&self) -> &'static str {
        "intent"
    }

    fn scope(&self, arguments: &Arguments) -> ScopeClaim {
        let intent = ArtifactClass::IntentContract.token();
        let claim = |intents: Vec<IntentHandle>, classes: Vec<&'static str>| ScopeClaim {
            snapshots: Vec::new(),
            intents,
            classes,
        };
        match arguments {
            Arguments::IntentGet(request) => claim(vec![request.intent.clone()], vec![intent]),
            Arguments::IntentDiff(request) => claim(
                vec![request.before.clone(), request.after.clone()],
                vec![intent, ArtifactClass::Diff.token()],
            ),
            Arguments::IntentProposeRevision(request) => {
                claim(vec![request.base.clone()], vec![intent])
            }
            Arguments::IntentAccept(request) => claim(
                vec![request.proposal.clone()],
                vec![intent, ArtifactClass::SignedIntentBundle.token()],
            ),
            Arguments::IntentReject(request) => claim(vec![request.proposal.clone()], vec![intent]),
            Arguments::IntentLock(request) => claim(vec![request.intent.clone()], vec![intent]),
            _ => ScopeClaim::default(),
        }
    }

    fn handle(
        &self,
        call: &Call<'_>,
        state: &mut DaemonState,
        services: &Services,
        _store: &ReferenceStore,
    ) -> Result<Effect, Fault> {
        match call.arguments {
            Arguments::IntentGet(request) => get(request, state),
            Arguments::IntentDiff(_) => Err(unclassifiable()),
            Arguments::IntentProposeRevision(request) => propose_revision(request, state),
            Arguments::IntentAccept(request) => accept(call, request, state),
            Arguments::IntentReject(request) => reject(request, state),
            Arguments::IntentLock(request) => lock(request, state, services),
            // Unreachable: the dispatcher checked shape agreement before routing.
            _ => Err(Fault::new(
                ErrorCode::MalformedRequest,
                "the request body is not the shape this operation declares",
            )),
        }
    }
}

fn get(request: &IntentGetRequest, state: &DaemonState) -> Result<Effect, Fault> {
    let record = state.intent(&request.intent).ok_or_else(Fault::denied)?;
    Ok(Effect::new(
        Payload::IntentGet(IntentGetResponse {
            intent: request.intent.clone(),
            contract: Opaque::from_bytes(record.contract.to_artifact_bytes()),
            record: Opaque::from_bytes(
                registry_record(&request.intent, record).to_canonical_bytes(),
            ),
        }),
        // `intent.get` declares no `verdict` clause, so its result carries none.
        Nullable::Null,
    ))
}

fn propose_revision(
    request: &IntentProposeRevisionRequest,
    state: &DaemonState,
) -> Result<Effect, Fault> {
    let record = state.intent(&request.base).ok_or_else(Fault::denied)?;
    let changes = Json::parse(request.changes.changes.as_bytes()).map_err(|_| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "the change set is not a canonical intent-contract document",
        )
    })?;
    let Json::Object(fields) = &changes else {
        return Err(Fault::new(
            ErrorCode::MalformedRequest,
            "the change set is not a canonical intent-contract document",
        ));
    };

    // The lock check runs before anything else this operation could refuse for, because a
    // locked field is the one refusal that is about the *contract* rather than about this
    // daemon's coverage — and because a caller told "unsupported" would retry.
    for key in fields.keys() {
        for field in touched(key) {
            if record.contract.policy().verb(field) == PolicyVerb::Locked {
                return Err(Fault::new(
                    ErrorCode::IntentMutationDenied,
                    "the change set names a field whose change policy is `locked`",
                ));
            }
        }
    }
    Err(unclassifiable())
}

fn accept(
    call: &Call<'_>,
    request: &IntentAcceptRequest,
    state: &mut DaemonState,
) -> Result<Effect, Fault> {
    // A bundle the daemon does not hold cannot have its chain verified, and plan §4.2.1
    // fails closed on exactly that: "CI fails closed with `AcceptanceChainInvalid` when the
    // referenced bundle is absent or its chain does not verify". This daemon holds no
    // bundles, so every named bundle is absent.
    if !request.bundle.is_absent() {
        return Err(Fault::new(
            ErrorCode::AcceptanceChainInvalid,
            "the acceptance names an intent bundle this daemon cannot verify a chain for",
        ));
    }
    let acceptance = decode_acceptance(&request.acceptance, call.audit.as_str())?;

    let record = state.intent(&request.proposal).ok_or_else(Fault::denied)?;
    if record.status != RegistryStatus::Proposed {
        return Err(Fault::new(
            ErrorCode::IntentMutationDenied,
            "only a proposed contract can be accepted into the registry",
        ));
    }

    let record = state
        .intent_mut(&request.proposal)
        .ok_or_else(Fault::denied)?;
    record.status = RegistryStatus::Accepted;
    record.acceptance = Some(acceptance);
    let stored = state.intent(&request.proposal).ok_or_else(Fault::denied)?;

    Ok(Effect::new(
        Payload::IntentAccept(IntentAcceptResponse {
            intent: request.proposal.clone(),
            record: Opaque::from_bytes(
                registry_record(&request.proposal, stored).to_canonical_bytes(),
            ),
        }),
        structural(StructuralOutcome::Accepted),
    ))
}

fn reject(request: &IntentRejectRequest, state: &mut DaemonState) -> Result<Effect, Fault> {
    let record = state.intent(&request.proposal).ok_or_else(Fault::denied)?;
    if record.status != RegistryStatus::Proposed {
        return Err(Fault::new(
            ErrorCode::IntentMutationDenied,
            "only a proposed contract can be rejected; an accepted one is protected",
        ));
    }
    state.drop_intent(&request.proposal);
    Ok(Effect::new(
        Payload::IntentReject(IntentRejectResponse {
            proposal: request.proposal.clone(),
        }),
        structural(StructuralOutcome::Rejected),
    ))
}

fn lock(
    request: &IntentLockRequest,
    state: &mut DaemonState,
    services: &Services,
) -> Result<Effect, Fault> {
    let record = state.intent(&request.intent).ok_or_else(Fault::denied)?;
    let contract = record.contract.clone();
    let acceptance = record.acceptance.clone();
    let current = contract.policy().clone();

    // Decode the edit against RFC 0037's two closed vocabularies. A key outside the fifteen
    // policy fields, a token outside the eight verbs, and a verb the W6 applicability matrix
    // does not admit on its field are all `MalformedRequest` — the code this operation's own
    // `errors` clause declares — because each is a document that does not validate against
    // the schema the field cites.
    let mut edited: BTreeMap<PolicyField, PolicyVerb> = current.iter().collect();
    for (key, token) in &request.policy {
        let field = PolicyField::from_wire(key).ok_or_else(|| {
            Fault::new(
                ErrorCode::MalformedRequest,
                "the policy table names a key outside the fifteen policy fields",
            )
        })?;
        let verb = PolicyVerb::from_wire(token).ok_or_else(|| {
            Fault::new(
                ErrorCode::MalformedRequest,
                "the policy table names a verb outside RFC 0037's closed set",
            )
        })?;
        // The lattice's own order decides. A privileged call may tighten a field and may
        // restate it; it may not weaken one, because a lock that a privileged call can lift
        // is not a lock (plan §5.4).
        if !current.verb(field).is_weaker_or_equal(verb) {
            return Err(Fault::new(
                ErrorCode::IntentMutationDenied,
                "the policy edit weakens a field's change policy",
            ));
        }
        edited.insert(field, verb);
    }
    let table = PolicyTable::new(edited).map_err(|_| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "the policy table is not well-formed for the fields it names",
        )
    })?;

    // ID3: a governance edit moves the identity, so the response names a successor.
    let successor = respell(&contract, table);
    let handle = mint(&successor, services)?;
    let successor = respell_id(&successor, &handle)?;
    let policy = wire_policy(successor.policy());
    let predecessor = request.intent.clone();

    if let Some(previous) = state.intent_mut(&predecessor) {
        previous.status = RegistryStatus::Superseded;
        previous.superseded_by = Some(handle.clone());
    }
    state.put_intent(
        handle.clone(),
        IntentRecord {
            contract: successor,
            status: RegistryStatus::Accepted,
            supersedes: Some(predecessor),
            superseded_by: None,
            acceptance,
        },
    );

    Ok(Effect::new(
        Payload::IntentLock(IntentLockResponse {
            intent: handle,
            policy,
        }),
        structural(StructuralOutcome::Locked),
    ))
}

/// The typed refusal both diff-shaped operations return.
fn unclassifiable() -> Fault {
    Fault::new(
        ErrorCode::UnsupportedSemanticFeature,
        "the RFC 0031 classification lane this operation's diff artifact comes from has \
         not shipped",
    )
}

/// The policy fields a top-level change-set key touches.
///
/// The change set is "in the form of `schemas/intent-contract.schema.json`", so its keys are
/// *contract paths*, not policy names — "a change under `claims` is reported as
/// `properties`" (RFC 0031). [`PolicyField::contract_path`] is that map, transcribed from
/// the schema's normative `x-policy-field-map`, so the translation is a lookup rather than a
/// second table. A key that is a path *prefix* touches every field beneath it, which is what
/// makes `optimization` reach `optimization.non_vacuity`.
fn touched(key: &str) -> impl Iterator<Item = PolicyField> + '_ {
    PolicyField::ALL.into_iter().filter(move |field| {
        let path = field.contract_path();
        path == key
            || path
                .strip_prefix(key)
                .is_some_and(|rest| rest.starts_with('.'))
    })
}

/// The acceptance record, validated against the schema's own `acceptance` object.
fn decode_acceptance(acceptance: &Opaque, audit: &str) -> Result<Acceptance, Fault> {
    let invalid = || {
        Fault::new(
            ErrorCode::AcceptanceChainInvalid,
            "the acceptance record does not verify against this daemon's policy",
        )
    };
    let Ok(Json::Object(fields)) = Json::parse(acceptance.as_bytes()) else {
        return Err(invalid());
    };
    let text = |key: &str| match fields.get(key) {
        Some(Json::String(value)) if !value.is_empty() => Some(value.clone()),
        _ => None,
    };
    // The schema fixes `capability` to the constant `revise-intent`; a record claiming any
    // other capability is not an acceptance record for this registry.
    if text("capability").as_deref() != Some(ACCEPTANCE_CAPABILITY) {
        return Err(invalid());
    }
    let accepted_by = text("accepted_by").ok_or_else(invalid)?;
    let signature = text("signature").ok_or_else(invalid)?;
    let timestamp = text("timestamp").ok_or_else(invalid)?;
    // The timestamp is the caller's — there is no clock here — but it is still held to the
    // one spelling the protocol fixes, so a record cannot carry a time nothing can order.
    Timestamp::new(&timestamp).map_err(|_| invalid())?;
    Ok(Acceptance {
        accepted_by,
        signature,
        timestamp,
        // Written by the daemon, never taken from the caller: plan §5.4 makes this the
        // record *the daemon* produced, and `rule audit.correlation` fixes its value.
        audit_record: audit.to_owned(),
    })
}

/// The registry record for one intent, in the form
/// `schemas/intent-registry-record.schema.json` declares.
///
/// `record_id` is the intent's own handle: the schema requires it to match the plan §4.4
/// handle pattern and does not require it to differ from `intent`, the registry holds
/// exactly one record per contract, and minting a second identity for a one-to-one relation
/// would be a second name for one thing.
fn registry_record(handle: &IntentHandle, record: &IntentRecord) -> Json {
    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
    fields.insert(
        "schema_id".to_owned(),
        Json::String(RECORD_SCHEMA_ID.to_owned()),
    );
    fields.insert(
        "schema_epoch".to_owned(),
        Json::Integer(RECORD_SCHEMA_EPOCH),
    );
    fields.insert(
        "record_id".to_owned(),
        Json::String(handle.as_str().to_owned()),
    );
    fields.insert(
        "intent".to_owned(),
        Json::String(handle.as_str().to_owned()),
    );
    fields.insert(
        "status".to_owned(),
        Json::String(record.status.as_wire().to_owned()),
    );
    fields.insert(
        "supersedes".to_owned(),
        handle_json(record.supersedes.as_ref()),
    );
    fields.insert(
        "superseded_by".to_owned(),
        handle_json(record.superseded_by.as_ref()),
    );
    if let Some(acceptance) = &record.acceptance {
        let mut inner: BTreeMap<String, Json> = BTreeMap::new();
        inner.insert(
            "accepted_by".to_owned(),
            Json::String(acceptance.accepted_by.clone()),
        );
        inner.insert(
            "capability".to_owned(),
            Json::String(ACCEPTANCE_CAPABILITY.to_owned()),
        );
        inner.insert(
            "signature".to_owned(),
            Json::String(acceptance.signature.clone()),
        );
        inner.insert(
            "audit_record".to_owned(),
            Json::String(acceptance.audit_record.clone()),
        );
        inner.insert(
            "timestamp".to_owned(),
            Json::String(acceptance.timestamp.clone()),
        );
        fields.insert("acceptance".to_owned(), Json::Object(inner));
    }
    Json::Object(fields)
}

fn handle_json(handle: Option<&IntentHandle>) -> Json {
    handle.map_or(Json::Null, |handle| {
        Json::String(handle.as_str().to_owned())
    })
}

/// The same contract with a different policy table.
fn respell(contract: &IntentContract, policy: PolicyTable) -> IntentContract {
    IntentContract::new(parts(contract, policy, contract.intent_id().clone()))
}

/// The same contract stamped with the identity it will be stored under.
///
/// ID2 excludes `intent_id` from the preimage, so this changes the document and not its
/// identity — which is what lets a successor be named before it carries its own name.
fn respell_id(contract: &IntentContract, handle: &IntentHandle) -> Result<IntentContract, Fault> {
    let id = IntentId::new(handle.as_str()).map_err(|_| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "the derived successor identity is not a well-formed intent handle",
        )
    })?;
    Ok(IntentContract::new(parts(
        contract,
        contract.policy().clone(),
        id,
    )))
}

fn parts(contract: &IntentContract, policy: PolicyTable, intent_id: IntentId) -> ContractParts {
    ContractParts {
        intent_id,
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
    }
}

/// The `in_*` handle a contract's ID1/ID2 preimage names.
fn mint(contract: &IntentContract, services: &Services) -> Result<IntentHandle, Fault> {
    let stored = services
        .identifier()
        .identify(
            ArtifactClass::IntentContract,
            &contract.identity_preimage_bytes(),
        )
        .map_err(|_| {
            Fault::new(
                ErrorCode::MalformedRequest,
                "no content identity could be derived for the successor contract",
            )
        })?;
    identity::intent_to_wire(&stored).map_err(|_| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "the derived successor identity is not a well-formed intent handle",
        )
    })
}

/// The policy table as the wire carries it: field wire name to verb wire token, all
/// fifteen. This is the round-trip half of `intent.lock` — what goes out is what a
/// following `intent.get` decodes from the stored contract.
fn wire_policy(table: &PolicyTable) -> BTreeMap<String, String> {
    table
        .iter()
        .map(|(field, verb)| (field.wire().to_owned(), verb.wire().to_owned()))
        .collect()
}

fn structural(outcome: StructuralOutcome) -> Nullable<Verdict> {
    Nullable::Value(Verdict::Structural(StructuralVerdictValue { outcome }))
}
