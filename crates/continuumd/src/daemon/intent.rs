//! The `intent` family: `get`, `diff`, `propose_revision`, `accept`, `reject`, `lock`, and,
//! from protocol 3.8, `export_bundle` and `import_bundle` (bn-3glnv).
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
//! A lock is never a road to protection (RFC 0037 correction 22, bn-10mth). It starts only
//! from the accepted head of a lineage that carries its acceptance block, and it mints only
//! an identity the registry does not hold; anything else is `IntentMutationDenied` with
//! nothing changed. `intent.accept` of a revision — a proposal naming a predecessor —
//! decides it first, on either path: the predecessor is the accepted head, and no field its
//! policy protects moves (the stand-in for RFC 0031's unshipped classifier that import also
//! applies). Accepting a revision supersedes the head, so a lineage keeps one.
//!
//! A lock's successor is protected by the lock, so its acceptance block names the lock —
//! the admitted actor, this daemon's time, and the lock's audit record — and never copies
//! its predecessor's (RFC 0037 correction 23, bn-1mgcv). A daemon that signs acceptances
//! signs that statement, so a peer's CI acceptance check verifies the lock actor honestly;
//! one that does not records no chain, and no peer honors the record. A local
//! `intent.accept` is held to the same admission, signed or not: `accepted_by` must be the
//! admitted actor and `timestamp` this daemon's own clock, or the acceptance is refused
//! before anything is written (correction 23 extended, bn-342ek) — closing the residual
//! correction 23 stated, that an unsigned local acceptance recorded the caller's say-so.
//!
//! # Bundles, and the one acceptance path that verifies a chain
//!
//! Plan §4.2.1's intent bundles arrive at protocol 3.8. `intent.export_bundle` signs the
//! named contracts, their registry records, the local allowed-signers set, and the signing
//! registry's audit log with the held key, and holds the result under its `inb_` identity.
//! `intent.import_bundle` reads a bundle under the bounds of [`bundle`](super::bundle),
//! recomputes every contract's `in_*` from its bytes (RFC 0037 I5), checks W1–W10 (I4) and
//! each revision's lineage and protected fields, and only then verifies the bundle. A
//! bundle that does not verify changes nothing. One that verifies has its standing facts
//! adopted, is held, and enters the contracts the registry does not hold at `proposed` —
//! never higher, whatever the bundle's records say (I2). Such a proposal is accepted only
//! through a bundle.
//!
//! `intent.accept` naming a `bundle` is the plan §4.2.1 CI acceptance check. It fails closed
//! with `AcceptanceChainInvalid` unless the bundle is held, its revocation records are a
//! prefix of this registry's, its signature passes the library's fail-closed
//! `verify_for_ci_acceptance` (ADR-0054), and it exports the proposal with the acceptance
//! the request presents over the proposal's own lineage
//! ([`SigningAuthority::check_acceptance_chain`]). An acceptance with no bundle is a local
//! acceptance: signed or not, it is held to the same admission — `accepted_by` the actor
//! this call admitted, `timestamp` this daemon's own clock reading — before it is written
//! (RFC 0037 correction 23 extended, bn-342ek).
//!
//! [`PolicyVerb::is_weaker_or_equal`]: continuum_intent::change_policy::PolicyVerb::is_weaker_or_equal
//! [`SigningAuthority::check_acceptance_chain`]: super::signing::SigningAuthority::check_acceptance_chain

use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{PolicyField, PolicyTable, PolicyVerb};
use continuum_intent::contract::{CheckEnvironment, ContractParts, IntentContract, IntentId};
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::ReferenceStore;

use super::admission::Derived;
use super::bundle::{BundleBody, BundleContract, SignedBundle, decode_signed, encode_signed};
use super::family::{Arguments, Call, Effect, Fault, OperationFamily, Payload, ScopeClaim};
use super::signing::{AcceptanceClaim, CustodyRefusal, require_signing_version};
use super::state::{Acceptance, DaemonState, IntentRecord, RegistryStatus};
use super::{Services, identity};
use crate::protocol::envelope::{StructuralVerdictValue, Verdict};
use crate::protocol::operations::intent::{
    IntentAcceptRequest, IntentAcceptResponse, IntentExportBundleRequest,
    IntentExportBundleResponse, IntentGetRequest, IntentGetResponse, IntentImportBundleRequest,
    IntentImportBundleResponse, IntentLockRequest, IntentLockResponse,
    IntentProposeRevisionRequest, IntentRejectRequest, IntentRejectResponse,
};
use crate::protocol::scalar::{IntentBundleHandle, IntentHandle, Opaque, Timestamp};
use crate::protocol::spec::Nullable;
use crate::protocol::vocabulary::{ErrorCode, SignatureOutcome, StructuralOutcome};

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
    ("intent.accept", ErrorCode::UnsupportedSemanticFeature),
    ("intent.reject", ErrorCode::CapabilityDenied),
    ("intent.reject", ErrorCode::IntentMutationDenied),
    ("intent.lock", ErrorCode::CapabilityDenied),
    ("intent.lock", ErrorCode::IntentMutationDenied),
    ("intent.lock", ErrorCode::MalformedRequest),
    ("intent.lock", ErrorCode::UnsupportedSemanticFeature),
    ("intent.export_bundle", ErrorCode::CapabilityDenied),
    ("intent.export_bundle", ErrorCode::MalformedRequest),
    ("intent.export_bundle", ErrorCode::PolicyGateFailed),
    ("intent.export_bundle", ErrorCode::QuotaExhausted),
    ("intent.export_bundle", ErrorCode::PublicationAborted),
    ("intent.import_bundle", ErrorCode::CapabilityDenied),
    ("intent.import_bundle", ErrorCode::MalformedRequest),
    ("intent.import_bundle", ErrorCode::QuotaExhausted),
    ("intent.import_bundle", ErrorCode::IntentMutationDenied),
    ("intent.import_bundle", ErrorCode::PublicationAborted),
    ("intent.import_bundle", ErrorCode::OutcomeUnknown),
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
            instances: Vec::new(),
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
            Arguments::IntentAccept(request) => ScopeClaim {
                // The optional bundle is an `inb_` instance the request names.
                instances: request
                    .bundle
                    .value()
                    .map(|bundle| bundle.as_str().to_owned())
                    .into_iter()
                    .collect(),
                ..claim(
                    vec![request.proposal.clone()],
                    vec![intent, ArtifactClass::SignedIntentBundle.token()],
                )
            },
            Arguments::IntentReject(request) => claim(vec![request.proposal.clone()], vec![intent]),
            Arguments::IntentLock(request) => claim(vec![request.intent.clone()], vec![intent]),
            // Both bundle operations also need an unscoped grant, which admission decides
            // by name (`admission::requires_unscoped_grant`).
            Arguments::IntentExportBundle(request) => claim(
                request.intents.clone(),
                vec![intent, ArtifactClass::SignedIntentBundle.token()],
            ),
            Arguments::IntentImportBundle(_) => claim(
                Vec::new(),
                vec![intent, ArtifactClass::SignedIntentBundle.token()],
            ),
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
            Arguments::IntentGet(request) => get(call, request, state),
            Arguments::IntentDiff(_) => Err(unclassifiable()),
            Arguments::IntentProposeRevision(request) => propose_revision(request, state),
            Arguments::IntentAccept(request) => accept(call, request, state, services),
            Arguments::IntentReject(request) => reject(request, state),
            Arguments::IntentLock(request) => lock(call, request, state, services),
            Arguments::IntentExportBundle(request) => export_bundle(call, request, state, services),
            Arguments::IntentImportBundle(request) => import_bundle(call, request, state, services),
            // Unreachable: the dispatcher checked shape agreement before routing.
            _ => Err(Fault::new(
                ErrorCode::MalformedRequest,
                "the request body is not the shape this operation declares",
            )),
        }
    }
}

/// The intents a registry record names beyond its own — `supersedes` and `superseded_by` —
/// decided against the grant before the record is reported or changed
/// (`rule capability.instance_scope`, the derived-handle clause; cr-3hcpn4). An
/// intent-scoped grant is not told the identity of a revision it does not list.
fn lineage_scope(call: &Call<'_>, record: &IntentRecord) -> Result<(), Fault> {
    for intent in [&record.supersedes, &record.superseded_by]
        .into_iter()
        .flatten()
    {
        call.derived(Derived::Intent(intent))?;
    }
    Ok(())
}

fn get(call: &Call<'_>, request: &IntentGetRequest, state: &DaemonState) -> Result<Effect, Fault> {
    let record = state.intent(&request.intent).ok_or_else(Fault::denied)?;
    lineage_scope(call, record)?;
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

/// The admission a local acceptance — one `intent.accept` writes with no bundle — must
/// meet before it is recorded, whether this daemon signs it or not: `accepted_by` must be
/// the actor this call admitted, and `timestamp` this daemon's own clock reading, never the
/// caller's unverified say-so (RFC 0037 A3; correction 23 named it for a lock successor and
/// a signed acceptance, and this extends it to an unsigned one, bn-342ek). The signed path
/// and the unsigned path share this one check, so neither can drift from the other.
///
/// # Errors
///
/// `UnsupportedSemanticFeature` when this deployment has no clock reading: an acceptance
/// that cannot honestly record its own time is refused, and nothing is written — the same
/// failure `intent.lock` already gives for the same reason (RFC 0037 correction 23). Before
/// bn-342ek the signed path answered a clockless deployment with `AcceptanceChainInvalid`
/// (its own admission check read a missing clock as a mismatch); sharing this one check
/// moves that case onto the fail-closed code too, which is the more honest answer: the
/// call was refused for want of a clock, not because the caller named the wrong principal
/// or time.
/// `AcceptanceChainInvalid` when `accepted_by` or `timestamp` disagrees with what this
/// daemon can attribute the acceptance to — the code the signed path already declared for
/// this before bn-342ek, and now the code every local acceptance shares.
fn require_admitted_locally(
    call: &Call<'_>,
    services: &Services,
    acceptance: &Acceptance,
) -> Result<(), Fault> {
    let now = services.now().ok_or_else(|| {
        Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "this deployment has no clock reading, and a local acceptance records its own \
             time; nothing was accepted",
        )
    })?;
    let admitted =
        acceptance.accepted_by == call.grant.actor.as_str() && acceptance.timestamp == now.as_str();
    if admitted {
        Ok(())
    } else {
        Err(Fault::new(
            ErrorCode::AcceptanceChainInvalid,
            "a local acceptance names the admitted principal and this daemon's time",
        ))
    }
}

fn accept(
    call: &Call<'_>,
    request: &IntentAcceptRequest,
    state: &mut DaemonState,
    services: &Services,
) -> Result<Effect, Fault> {
    // A bundle the daemon does not hold cannot have its chain verified, and plan §4.2.1
    // fails closed on exactly that: "CI fails closed with `AcceptanceChainInvalid` when the
    // referenced bundle is absent or its chain does not verify". Below 3.8 no bundle can
    // verify, so a connection negotiated there keeps the 3.0–3.7 answer for every named
    // bundle.
    // While the signing custody is unreconciled, the live signing state may not be what a
    // restart loads, and it decides whether an acceptance is signed, by which key, and
    // whether a bundle's chain holds. No acceptance is decided from it (review cr-1dc5ii).
    let bundle = request.bundle.value();
    if state.signing().custody_unreconciled() {
        // A bundle acceptance is the CI check, and it fails closed on its own code.
        return Err(if bundle.is_some() {
            chain_invalid()
        } else {
            unreconciled_custody()
        });
    }
    let legacy = services.negotiated().protocol_version() < super::signing::SIGNING_SINCE;
    if bundle.is_some_and(|handle| legacy || state.signing().bundle(handle).is_none()) {
        return Err(chain_invalid());
    }
    let mut acceptance = decode_acceptance(&request.acceptance, call.audit.as_str())?;

    let record = state.intent(&request.proposal).ok_or_else(Fault::denied)?;
    lineage_scope(call, record)?;
    if record.status != RegistryStatus::Proposed {
        return Err(Fault::new(
            ErrorCode::IntentMutationDenied,
            "only a proposed contract can be accepted into the registry",
        ));
    }

    // A proposal that names a predecessor is a revision, and a revision reaches protection
    // only after its change set is decided against the predecessor's policy table (RFC 0037
    // R4, P1–P6; bn-10mth). RFC 0031's classifier has not shipped, so the decision is the
    // same stand-in import applies (correction 19): the predecessor is this registry's
    // accepted lineage head, no field its policy does not mark `unlocked` moves, and the
    // policy table does not weaken. Anything else is refused before anything is signed or
    // written. A bundle acceptance answers "not the head" with its own A2 code below; a
    // local acceptance of an imported proposal is refused by its own arm below.
    let supersedes = record.supersedes.clone();
    let imported = state.signing().imported_from(&request.proposal).is_some();
    if let Some(predecessor) = &supersedes {
        let held = state.intent(predecessor).filter(|held| accepted_head(held));
        match held {
            Some(held) if !respects_predecessor(&held.contract, &record.contract) => {
                return Err(unclassified_revision());
            }
            None if bundle.is_none() && !imported => {
                return Err(Fault::new(
                    ErrorCode::IntentMutationDenied,
                    "a revision is accepted only over its lineage's accepted head",
                ));
            }
            _ => {}
        }
    }

    // The plan §4.2.1 CI acceptance check, failing closed (`rule intent.bundles`): the held
    // bundle's signature chain, under this registry and local policy, through
    // the library's fail-closed `verify_for_ci_acceptance`. Every failure is the one code.
    match bundle {
        Some(bundle) => {
            let local = record.contract.to_artifact_bytes();
            let claim = AcceptanceClaim {
                contract: &local,
                accepted_by: &acceptance.accepted_by,
                signature: &acceptance.signature,
                timestamp: &acceptance.timestamp,
                supersedes: supersedes.as_ref(),
            };
            let actor = super::signing::audit_actor(call)?;
            let checked =
                state
                    .signing()
                    .check_acceptance_chain(bundle, &request.proposal, &claim, &actor);
            match checked {
                Ok(chain) => acceptance.chain = chain,
                Err(fault) => {
                    if fault == super::signing::AcceptanceFault::IdentityCollision {
                        state.signing_mut().note_identity_collision();
                    }
                    return Err(chain_invalid());
                }
            }
            // A2 against this registry, not against the bundle: the predecessor the
            // acceptance names must be the accepted head of a lineage this daemon holds,
            // so a signed acceptance is never replayed onto a lineage that exists only in
            // the bundle.
            if let Some(predecessor) = &supersedes {
                let head = state.intent(predecessor).is_some_and(accepted_head);
                if !head {
                    return Err(chain_invalid());
                }
            }
        }
        // A proposal an import entered came from someone else's registry: it is accepted
        // only through a bundle that verifies, never by a local acceptance.
        None if imported => {
            return Err(chain_invalid());
        }
        // A local acceptance at 3.8 is signed by this daemon's key when local policy allows
        // it to sign acceptances: the A1 statement over this record, which a peer verifies
        // when it accepts through a bundle. The caller's `signature` text is then replaced
        // by that signature, as nothing could verify the caller's text.
        //
        // What the key signs is what this daemon admitted, never the caller's say-so: the
        // principal must be the admitted actor, and the time this daemon's own reading.
        // The key vouches that this daemon admitted that principal, holding the
        // `revise-intent` privilege, at that time; a peer that allows the key for
        // `intent-acceptance` trusts this daemon's admission (RFC 0037 A3).
        None if !legacy && state.signing().signs_acceptances() => {
            require_admitted_locally(call, services, &acceptance)?;
            let statement = super::acceptance::Statement {
                intent: &request.proposal,
                base: supersedes.as_ref(),
                accepted_by: &acceptance.accepted_by,
                timestamp: &acceptance.timestamp,
            };
            let element = state.signing().sign_acceptance(&statement).map_err(|()| {
                Fault::new(
                    ErrorCode::UnsupportedSemanticFeature,
                    "this daemon's acceptance-signing key is not active; nothing was accepted",
                )
            })?;
            if let Some(element) = element {
                acceptance.signature.clone_from(&element.signature);
                acceptance.chain = vec![element];
            }
        }
        // Without such a key, or below 3.8, this daemon signs nothing — but it still writes
        // a registry fact naming who accepted and when, and that fact is this daemon's own
        // to misattribute or not. It is held to the identical admission the signed arm
        // above checks (RFC 0037 correction 23 extended, bn-342ek): the record carries no
        // chain, so no peer's CI acceptance check will ever honor it, but this daemon's own
        // registry does, and a caller does not get to name someone else, or another time,
        // as the one this daemon admitted.
        None => {
            require_admitted_locally(call, services, &acceptance)?;
        }
    }

    // Accepting a revision supersedes the head it revises, on either path, so a lineage
    // keeps one accepted head (A2, RFC 0037 R6).
    if let Some(predecessor) = &supersedes {
        if let Some(previous) = state.intent_mut(predecessor) {
            previous.status = RegistryStatus::Superseded;
            previous.superseded_by = Some(request.proposal.clone());
        }
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
    state.signing_mut().forget_import(&request.proposal);
    Ok(Effect::new(
        Payload::IntentReject(IntentRejectResponse {
            proposal: request.proposal.clone(),
        }),
        structural(StructuralOutcome::Rejected),
    ))
}

fn lock(
    call: &Call<'_>,
    request: &IntentLockRequest,
    state: &mut DaemonState,
    services: &Services,
) -> Result<Effect, Fault> {
    let record = state.intent(&request.intent).ok_or_else(Fault::denied)?;
    lineage_scope(call, record)?;
    // A lock edits the policy table of a contract that is *already* protected; it is never
    // a way to reach protection (bn-10mth). RFC 0037: "The `proposed` → protected
    // transition is performed only by the `intent.accept` operation", and a proposal is
    // accepted only after RFC 0031's classifier has decided its change set and A1–A4 have
    // run. So the one contract a lock may start from is the accepted head of a lineage that
    // carries its acceptance block: not a proposal, local or imported (correction 19), whose
    // successor would be accepted with no acceptance at all; and not a superseded record,
    // whose successor would fork the lineage and copy an acceptance that no longer governs.
    if !accepted_head(record) {
        return Err(Fault::new(
            ErrorCode::IntentMutationDenied,
            "only the accepted head of a lineage can be locked; a proposal is protected only \
             through intent.accept",
        ));
    }
    let contract = record.contract.clone();
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
    // The same lineage decision every other road to an accepted successor makes. A lock
    // moves only verbs, so this holds by construction; it is checked, not assumed.
    if !respects_predecessor(&contract, &successor) {
        return Err(unclassified_revision());
    }
    let handle = mint(&successor, services)?;
    // The successor's identity is a function of the edited contract, so it may be one this
    // daemon already holds, and `put_intent` below writes it. It is decided by the grant's
    // `intents` list first, held or not (X2; cr-3hcpn4).
    call.derived(Derived::Intent(&handle))?;
    // The successor must be new. A held record under that identity is either the
    // predecessor itself (an edit that changes nothing) or another record — a proposal
    // among them — which the write below would overwrite as `accepted` with the
    // predecessor's acceptance, around `intent.accept` (bn-10mth). Decided after the grant
    // check above, so a refusal here says nothing about a handle outside the grant.
    if state.intent(&handle).is_some() {
        return Err(Fault::new(
            ErrorCode::IntentMutationDenied,
            "the lock's successor is already held; a lock mints a new contract or nothing",
        ));
    }
    let successor = respell_id(&successor, &handle)?;
    let policy = wire_policy(successor.policy());
    let predecessor = request.intent.clone();
    // Decided before anything is written, so a refusal here changes nothing either.
    let acceptance = lock_acceptance(call, services, state, &handle, &predecessor)?;

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
            acceptance: Some(acceptance),
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

/// The `signature` text of a lock successor's acceptance when this daemon signs no
/// acceptances (below protocol 3.8, or with no key local policy allows to sign one). It is
/// not a signature and names no one; the record then carries no chain, so no other
/// daemon's CI acceptance check honors it (RFC 0037 correction 23).
pub(crate) const UNSIGNED_LOCK: &str = "unsigned";

/// The acceptance block of the successor that a lock of `predecessor` mints as `successor`
/// (RFC 0037 correction 23, bn-1mgcv).
///
/// The successor is protected by the lock, so its record names the lock: the admitted
/// actor that performed it, this daemon's own time reading, and the lock's own audit
/// record. It never copies the predecessor's acceptance, which is a statement about
/// another contract by a principal who did not perform this act. When this daemon signs
/// acceptances, it signs the A1 statement over the successor and its predecessor as a
/// one-element chain, which a peer's CI acceptance check verifies against the lock actor
/// and time. A predecessor's chain is never carried over, because every element of a chain
/// signs the statement of its own contract (correction 21).
///
/// # Errors
///
/// `UnsupportedSemanticFeature` when the deployment has no clock reading, or its
/// acceptance-signing key is no longer active: a lock that cannot record its own act
/// honestly is refused, and nothing is written.
fn lock_acceptance(
    call: &Call<'_>,
    services: &Services,
    state: &DaemonState,
    successor: &IntentHandle,
    predecessor: &IntentHandle,
) -> Result<Acceptance, Fault> {
    if state.signing().custody_unreconciled() {
        return Err(unreconciled_custody());
    }
    let now = services.now().ok_or_else(|| {
        Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "this deployment has no clock reading, and a lock records its own time; nothing \
             was locked",
        )
    })?;
    let mut acceptance = Acceptance {
        accepted_by: call.grant.actor.as_str().to_owned(),
        signature: UNSIGNED_LOCK.to_owned(),
        timestamp: now.as_str().to_owned(),
        audit_record: call.audit.as_str().to_owned(),
        chain: Vec::new(),
    };
    if services.negotiated().protocol_version() < super::signing::SIGNING_SINCE {
        return Ok(acceptance);
    }
    let statement = super::acceptance::Statement {
        intent: successor,
        base: Some(predecessor),
        accepted_by: &acceptance.accepted_by,
        timestamp: &acceptance.timestamp,
    };
    let element = state.signing().sign_acceptance(&statement).map_err(|()| {
        Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "this daemon's acceptance-signing key is not active; nothing was locked",
        )
    })?;
    if let Some(element) = element {
        acceptance.signature.clone_from(&element.signature);
        acceptance.chain = vec![element];
    }
    Ok(acceptance)
}

/// Whether `record` is the accepted head of its lineage, holding the acceptance block that
/// `intent.accept` recorded (RFC 0037, "the `proposed` → protected transition is performed
/// only by the `intent.accept` operation"). It is what `intent.lock` may start from, what
/// an accepted revision may supersede, and what `workspace.create` may bind a new snapshot
/// to; a record without its acceptance block fails closed.
pub(crate) fn accepted_head(record: &IntentRecord) -> bool {
    record.status == RegistryStatus::Accepted
        && record.superseded_by.is_none()
        && record.acceptance.is_some()
}

fn chain_invalid() -> Fault {
    Fault::new(
        ErrorCode::AcceptanceChainInvalid,
        "the acceptance chain does not verify against this daemon's registry and policy",
    )
    .not_retryable()
}

// --- intent bundles (protocol 3.8, bn-3glnv) ----------------------------------------------

/// A registry record a bundle carries, read for exactly what import and acceptance need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BundleRecord {
    status: RegistryStatus,
    supersedes: Option<IntentHandle>,
    acceptance: Option<(String, String, String)>,
    chain: Vec<super::acceptance::ChainElement>,
}

impl BundleRecord {
    /// Read a bundle's record for `intent` against
    /// `schemas/intent-registry-record.schema.json`'s header and the fields import and
    /// acceptance read. The bytes were bounded by the bundle decoder before this runs.
    ///
    /// # Errors
    ///
    /// `()` for any record that does not name `intent`, carries another schema header, an
    /// unknown status, a malformed `supersedes` or `superseded_by`, an `accepted` status
    /// without its acceptance block (RFC 0037: such a bundle "is malformed and MUST be
    /// rejected"), or a `superseded_by` present exactly when the status is not `superseded`.
    pub(crate) fn parse(bytes: &[u8], intent: &IntentHandle) -> Result<Self, ()> {
        let Ok(Json::Object(fields)) = Json::parse(bytes) else {
            return Err(());
        };
        let text = |key: &str| match fields.get(key) {
            Some(Json::String(value)) => Some(value.as_str()),
            _ => None,
        };
        if text("schema_id") != Some(RECORD_SCHEMA_ID)
            || fields.get("schema_epoch") != Some(&Json::Integer(RECORD_SCHEMA_EPOCH))
            || text("intent") != Some(intent.as_str())
        {
            return Err(());
        }
        let status = match text("status") {
            Some("proposed") => RegistryStatus::Proposed,
            Some("accepted") => RegistryStatus::Accepted,
            Some("superseded") => RegistryStatus::Superseded,
            _ => return Err(()),
        };
        let supersedes = match fields.get("supersedes") {
            None | Some(Json::Null) => None,
            Some(Json::String(value)) => Some(IntentHandle::new(value).map_err(|_| ())?),
            Some(_) => return Err(()),
        };
        let acceptance = match fields.get("acceptance") {
            None => None,
            Some(Json::Object(inner)) => {
                let field = |key: &str| match inner.get(key) {
                    Some(Json::String(value)) if !value.is_empty() => Ok(value.clone()),
                    _ => Err(()),
                };
                if field("capability")? != ACCEPTANCE_CAPABILITY {
                    return Err(());
                }
                Some((
                    field("accepted_by")?,
                    field("signature")?,
                    field("timestamp")?,
                ))
            }
            Some(_) => return Err(()),
        };
        if status == RegistryStatus::Accepted && acceptance.is_none() {
            return Err(());
        }
        // The lineage edge the schema ties to status (bn-1mgcv): a `superseded` record names
        // its successor, and a record that names one is not an accepted head, whatever its
        // status says. Either inconsistency is a malformed record, not an acceptance.
        let names_successor = match fields.get("superseded_by") {
            None | Some(Json::Null) => false,
            Some(Json::String(value)) => {
                IntentHandle::new(value).map_err(|_| ())?;
                true
            }
            Some(_) => return Err(()),
        };
        if names_successor != (status == RegistryStatus::Superseded) {
            return Err(());
        }
        // The chain is read, bounded, and kept verbatim here; it is verified only when an
        // acceptance names the bundle (RFC 0037 A3, `check_acceptance_chain`).
        let chain = match fields.get("chain") {
            None => Vec::new(),
            Some(Json::Array(items)) if items.len() <= super::acceptance::MAX_CHAIN => items
                .iter()
                .map(|item| {
                    let Json::Object(element) = item else {
                        return Err(());
                    };
                    if element.len() != 3 {
                        return Err(());
                    }
                    let field = |key: &str| match element.get(key) {
                        Some(Json::String(value)) => Ok(value.clone()),
                        _ => Err(()),
                    };
                    Ok(super::acceptance::ChainElement {
                        signer: field("signer")?,
                        signature: field("signature")?,
                        scope: field("scope")?,
                    })
                })
                .collect::<Result<Vec<_>, ()>>()?,
            Some(_) => return Err(()),
        };
        Ok(Self {
            status,
            supersedes,
            acceptance,
            chain,
        })
    }

    /// The record's signature chain, oldest first.
    pub(crate) fn chain(&self) -> &[super::acceptance::ChainElement] {
        &self.chain
    }

    /// The acceptance block's `signature`, when there is one.
    pub(crate) fn acceptance_signature(&self) -> Option<&str> {
        self.acceptance
            .as_ref()
            .map(|(_, signature, _)| signature.as_str())
    }

    /// Whether this signed record accepts the contract exactly as `claim` presents it: status
    /// `accepted`, the same principal, signature, and time, and the same lineage predecessor
    /// as the local record (RFC 0037 A2: an acceptance replayed onto another base is refused).
    pub(crate) fn accepts(&self, claim: &AcceptanceClaim<'_>) -> bool {
        self.status == RegistryStatus::Accepted
            && self.supersedes.as_ref() == claim.supersedes
            && self.acceptance.as_ref().is_some_and(|(by, signature, at)| {
                by == claim.accepted_by && signature == claim.signature && at == claim.timestamp
            })
    }
}

fn malformed_bundle() -> Fault {
    Fault::new(
        ErrorCode::MalformedRequest,
        "the content is not a well-formed intent bundle within its bounds",
    )
    .not_retryable()
}

/// The refusal for a content-addressed handle that already names byte-different content
/// (ADR-0013: collisions resolve by exact comparison). Deterministic, so not retryable.
fn identity_collision() -> Fault {
    Fault::new(
        ErrorCode::PublicationAborted,
        "an identity already names byte-different content; nothing changed",
    )
    .not_retryable()
}

fn bundle_handle(services: &Services, content: &[u8]) -> Result<IntentBundleHandle, Fault> {
    let stored = services
        .identifier()
        .identify(ArtifactClass::SignedIntentBundle, content)
        .map_err(|_| {
            Fault::new(
                ErrorCode::MalformedRequest,
                "no content identity could be derived for the bundle",
            )
        })?;
    identity::bundle_to_wire(&stored)
        .ok()
        .filter(|handle| handle.as_str().len() <= super::bundle::MAX_BUNDLE_HANDLE_LEN)
        .ok_or_else(|| {
            Fault::new(
                ErrorCode::MalformedRequest,
                "the derived bundle identity is not a well-formed bundle handle within its bound",
            )
        })
}

/// `intent.export_bundle`: sign the named contracts, their records, the local allowed set,
/// and the registry's audit log with the held key; hold the bundle and return it.
fn export_bundle(
    call: &Call<'_>,
    request: &IntentExportBundleRequest,
    state: &mut DaemonState,
    services: &Services,
) -> Result<Effect, Fault> {
    require_signing_version(services)?;
    // Bounded before the set is built.
    if request.intents.len() > super::bundle::MAX_BUNDLE_CONTRACTS {
        return Err(Fault::new(
            ErrorCode::MalformedRequest,
            "an export names between one and the bundle bound of contracts",
        )
        .not_retryable());
    }
    let intents: std::collections::BTreeSet<&IntentHandle> = request.intents.iter().collect();
    if intents.is_empty() {
        return Err(Fault::new(
            ErrorCode::MalformedRequest,
            "an export names between one and the bundle bound of contracts",
        )
        .not_retryable());
    }
    // The cheap refusals first: a key that may sign bundles, and room to hold one.
    let authority = state.signing();
    if !authority.may_sign(continuum_evidence::signing::SignedArtifactKind::IntentBundle) {
        return Err(super::signing::no_active_key().not_retryable());
    }
    if !authority.has_room_for_more() {
        return Err(Fault::new(
            ErrorCode::QuotaExhausted,
            "this daemon holds its bound of intent bundles",
        )
        .not_retryable());
    }
    let pins = authority.export_pins();
    if pins.len() > super::bundle::MAX_ALLOWED_SIGNERS {
        return Err(Fault::new(
            ErrorCode::QuotaExhausted,
            "local policy pins more active bundle signers than a bundle carries",
        )
        .not_retryable());
    }
    let oversize = || {
        Fault::new(
            ErrorCode::MalformedRequest,
            "the bundle would exceed a bundle bound; nothing was exported",
        )
        .not_retryable()
    };
    let mut contracts = Vec::with_capacity(intents.len());
    for handle in intents {
        let record = state.intent(handle).ok_or_else(Fault::denied)?;
        lineage_scope(call, record)?;
        let entry = BundleContract {
            intent: handle.clone(),
            contract: record.contract.to_artifact_bytes(),
            record: registry_record(handle, record).to_canonical_bytes(),
        };
        if entry.contract.len() + entry.record.len() > super::bundle::MAX_CONTRACT_ENTRY_LEN - 64
            || entry.record.len() > super::bundle::MAX_RECORD_JSON_LEN
        {
            return Err(oversize());
        }
        contracts.push(entry);
    }
    let authority = state.signing();
    let body = BundleBody {
        allowed: pins,
        contracts,
        links: authority.export_links()?,
    };
    let body_bytes = body.encode();
    // Bounded before it is signed.
    if body_bytes.len() > super::bundle::MAX_BUNDLE_LEN - 1024 {
        return Err(oversize());
    }
    let signature = authority.sign_bundle(&body_bytes)?;
    let content = encode_signed(&body_bytes, &signature.encode());
    // What this daemon writes, it must be able to read back: an export over any bound is
    // refused here rather than handed to an importer that would refuse it.
    let signed = decode_signed(&content).map_err(|_| oversize())?;
    let handle = bundle_handle(services, &content)?;
    call.derived(Derived::Instance(handle.as_str()))?;
    if state.signing().holds_other_bundle(&handle, &signed) {
        state.signing_mut().note_identity_collision();
        return Err(identity_collision());
    }
    if !state
        .signing()
        .has_room_for_bundle(&handle, signed.charged_len())
    {
        return Err(Fault::new(
            ErrorCode::QuotaExhausted,
            "this daemon holds its bound of intent bundles",
        )
        .not_retryable());
    }
    state.signing_mut().hold(handle.clone(), signed);
    Ok(Effect::new(
        Payload::IntentExportBundle(IntentExportBundleResponse {
            bundle: handle,
            content,
        }),
        structural(StructuralOutcome::Created),
    ))
}

/// `intent.import_bundle`: validate every layer and every contract, and verify the bundle,
/// before anything changes; then adopt the standing facts it carries, enter the contracts
/// the registry does not hold at `proposed`, and hold it. A bundle that does not verify
/// changes nothing and is not held. Idempotent (RFC 0037 I1).
fn import_bundle(
    call: &Call<'_>,
    request: &IntentImportBundleRequest,
    state: &mut DaemonState,
    services: &Services,
) -> Result<Effect, Fault> {
    require_signing_version(services)?;
    super::signing::require_custody_version(services, state.signing())?;
    // Checked first, before any decoding: while custody is unreconciled nothing verifies,
    // so the answer is known (review cr-1dc5ii). `verify_bundle` checks again.
    if state.signing().custody_unreconciled() {
        return Err(unreconciled_custody());
    }
    // Bounded before any byte is decoded (`decode_signed` checks the length first).
    let signed = decode_signed(&request.content).map_err(|_| malformed_bundle())?;
    // A bundle whose signature authenticates its body is then held to its links before
    // anything else: a standing change is adopted only on the word of the keys it
    // concerns, so a link one of them did not sign, or a key rotated twice, makes the
    // bundle malformed. A bundle that does not authenticate adopts nothing anyway, and its
    // typed outcome comes from verification below.
    let authentic = continuum_evidence::signing::ArtifactSignature::decode(signed.signature())
        .is_ok_and(|signature| {
            signature.authenticates(
                continuum_evidence::signing::SignedArtifactKind::IntentBundle,
                &super::bundle::signed_bytes_identity(signed.body_bytes()),
            )
        });
    if authentic && !super::signing::links_are_attested(&signed.body().links) {
        state.signing_mut().note_unattested_link();
        return Err(Fault::new(
            ErrorCode::MalformedRequest,
            "a carried signer link is not signed by every key it concerns; nothing changed",
        )
        .not_retryable());
    }

    // I4, I5: every contract parses, is well formed, and is the identity it claims to be.
    // Each is read on its own, bounded by the entry bound the decoder already enforced.
    let mut entries: Vec<(IntentHandle, IntentContract, BundleRecord)> =
        Vec::with_capacity(signed.body().contracts.len());
    for entry in &signed.body().contracts {
        let contract = IntentContract::decode(&entry.contract).map_err(|_| malformed_bundle())?;
        // One spelling per contract: the entry is the canonical bytes of what it decodes to,
        // so comparing bytes below compares contracts.
        if contract.to_artifact_bytes() != entry.contract {
            return Err(malformed_bundle());
        }
        let recomputed = mint(&contract, services)?;
        if recomputed != entry.intent {
            return Err(malformed_bundle());
        }
        // I4: W1–W10 against what an import can supply — the `in_` this registry minted
        // and nothing else. There is no snapshot here to resolve a domain-pack profile or a
        // correspondence map against, so a contract naming one fails W8 and is refused:
        // fail closed, never stored partially.
        let environment = CheckEnvironment::new().with_minted_intent_id(entry.intent.as_str());
        if !contract.check(&environment).is_well_formed() {
            return Err(malformed_bundle());
        }
        let record =
            BundleRecord::parse(&entry.record, &entry.intent).map_err(|_| malformed_bundle())?;
        entries.push((entry.intent.clone(), contract, record));
    }

    let handle = bundle_handle(services, &request.content)?;
    call.derived(Derived::Instance(handle.as_str()))?;
    let actor = super::signing::audit_actor(call)?;

    // A handle already held names these exact bytes, or the import is refused: an `inb_*`
    // or `in_*` that names byte-different content is an identity collision (ADR-0013), and
    // is refused before any fact is adopted, any contract enters, or any bundle is held.
    // Re-importing the same bytes stays idempotent (I1, I6).
    let collides = state.signing().holds_other_bundle(&handle, &signed)
        || signed.body().contracts.iter().any(|entry| {
            state
                .intent(&entry.intent)
                .is_some_and(|held| held.contract.to_artifact_bytes() != entry.contract)
        });
    if collides {
        state.signing_mut().note_identity_collision();
        return Err(identity_collision());
    }

    // The contracts this import would enter, each checked against its lineage before
    // anything is written: the predecessor a record names is held here or exported by the
    // same bundle, and no field its policy protects moves, nor does the policy weaken
    // (INV-001, plan §5.4). The RFC 0031 classifier has not shipped, so any change to a
    // field whose verb is not `unlocked` is refused rather than classified.
    let mut fresh = Vec::new();
    for (intent, contract, record) in &entries {
        if state.intent(intent).is_some() {
            continue;
        }
        call.derived(Derived::Intent(intent))?;
        if let Some(predecessor) = &record.supersedes {
            call.derived(Derived::Intent(predecessor))?;
            let before = state
                .intent(predecessor)
                .map(|held| &held.contract)
                .or_else(|| {
                    entries
                        .iter()
                        .find(|(other, _, _)| other == predecessor)
                        .map(|(_, other, _)| other)
                })
                .ok_or_else(malformed_bundle)?;
            if !respects_predecessor(before, contract) {
                return Err(Fault::new(
                    ErrorCode::IntentMutationDenied,
                    "an imported revision changes a field its predecessor's policy protects",
                ));
            }
        }
        fresh.push(intent.clone());
    }

    // Verify against the registry with the bundle's facts applied, changing nothing yet.
    // An unverified bundle leaves the daemon exactly as it was.
    let adoption = match state.signing().verify_bundle(&signed, &actor) {
        Ok(adoption) => adoption,
        Err(outcome) => {
            return Ok(Effect::new(
                Payload::IntentImportBundle(IntentImportBundleResponse {
                    bundle: handle,
                    outcome,
                    imported: Vec::new(),
                    adopted: 0,
                }),
                structural(StructuralOutcome::Unchanged),
            ));
        }
    };
    let outcome = SignatureOutcome::Verified;
    let charged = signed.charged_len();
    if !state.signing().has_room_for_bundle(&handle, charged) {
        return Err(Fault::new(
            ErrorCode::QuotaExhausted,
            "this daemon holds its bound of intent bundles",
        )
        .not_retryable());
    }

    // The facts, the bundle, and the import records of the contracts it enters are
    // recorded through the custody as one write first (bn-3snfi); a refusal there changes
    // nothing.
    let (adopted, held, recorded) = state
        .signing_mut()
        .commit_import(adoption, &handle, signed, &fresh)?;
    // Nothing below can fail: the contracts enter with everything else, or not at all. An
    // unconfirmed write keeps the change, like every unconfirmed signing change, and the
    // answer below is `OutcomeUnknown`.
    for (intent, contract, record) in entries {
        if fresh.contains(&intent) {
            state.put_intent(
                intent,
                IntentRecord {
                    contract,
                    // I2: import never raises protection status, whatever the record says.
                    status: RegistryStatus::Proposed,
                    supersedes: record.supersedes,
                    superseded_by: None,
                    acceptance: None,
                },
            );
        }
    }
    recorded.answer()?;
    let changed = held || adopted > 0 || !fresh.is_empty();
    Ok(Effect::new(
        Payload::IntentImportBundle(IntentImportBundleResponse {
            bundle: handle,
            outcome,
            imported: fresh,
            adopted,
        }),
        structural(if changed {
            StructuralOutcome::Created
        } else {
            StructuralOutcome::Unchanged
        }),
    ))
}

/// What a restart restores from the custody's held bundles and import records (bn-3snfi),
/// each checked by [`restore_imports`].
#[derive(Debug)]
pub(crate) struct RestoredImports {
    /// The held bundles, decoded, by their recomputed identities.
    pub(crate) bundles: Vec<(IntentBundleHandle, SignedBundle)>,
    /// The import records.
    pub(crate) records: BTreeMap<IntentHandle, IntentBundleHandle>,
    /// The contract each record re-enters at `proposed`, read from its bundle.
    pub(crate) intents: Vec<(IntentHandle, IntentRecord)>,
}

/// Check the held bundles and import records a restored custody state carries, before any
/// of them is used or anything is swept (bn-3snfi), and read what they re-enter.
///
/// Every held bundle is within the held-bundle bounds (charged, count and bytes, before
/// any is decoded), decodes under [`decode_signed`]'s bounds, is the identity it is
/// recorded under (recomputed from its bytes), is authenticated by its own signature, and
/// carries only links every key they concern signed — what import checked before it held
/// the bundle. Every import record names a held bundle that exports the contract, whose
/// bytes are canonical, recompute to its `in_*` (RFC 0037 I5), pass W1–W10 as import
/// checks them (I4), and carry a well-formed registry record. Every adopted link is carried
/// by a held bundle, because an adoption holds its bundle in the same write. Signer
/// standing is not re-checked here: a bundle whose signer was revoked after the import
/// stays held, and `intent.accept` re-verifies its chain, failing closed.
///
/// A re-entered contract is `proposed`, never higher (I2), whatever it was before the
/// restart: the registry itself is not persisted, so a restart re-enters exactly what
/// importing each recorded bundle again would.
///
/// # Errors
///
/// [`CustodyRefusal::HeldBundle`], [`CustodyRefusal::ImportRecord`], or
/// [`CustodyRefusal::UncarriedLink`]; nothing is repaired.
pub(crate) fn restore_imports(
    services: &Services,
    bundles: &BTreeMap<String, std::sync::Arc<[u8]>>,
    imports: &BTreeMap<String, String>,
    adopted: &[continuum_evidence::signing::SignerLink],
) -> Result<RestoredImports, CustodyRefusal> {
    use continuum_evidence::signing::{ArtifactSignature, SignedArtifactKind};
    // Charged before anything is decoded: the count, then every length together.
    let bytes = bundles
        .values()
        .fold(0usize, |total, content| total.saturating_add(content.len()));
    if bundles.len() > super::signing::MAX_HELD_BUNDLES
        || bytes > super::signing::MAX_HELD_BUNDLE_BYTES
        || imports.len() > super::signing::MAX_HELD_BUNDLES * super::bundle::MAX_BUNDLE_CONTRACTS
    {
        return Err(CustodyRefusal::HeldBundle);
    }
    let mut held: BTreeMap<IntentBundleHandle, SignedBundle> = BTreeMap::new();
    for (name, content) in bundles {
        let handle = IntentBundleHandle::new(name).map_err(|_| CustodyRefusal::HeldBundle)?;
        let signed = super::bundle::decode_signed_shared(std::sync::Arc::clone(content))
            .map_err(|_| CustodyRefusal::HeldBundle)?;
        let recomputed =
            bundle_handle(services, content).map_err(|_| CustodyRefusal::HeldBundle)?;
        if recomputed != handle {
            return Err(CustodyRefusal::HeldBundle);
        }
        let authentic = ArtifactSignature::decode(signed.signature()).is_ok_and(|signature| {
            signature.authenticates(
                SignedArtifactKind::IntentBundle,
                &super::bundle::signed_bytes_identity(signed.body_bytes()),
            )
        });
        if !authentic || !super::signing::links_are_attested(&signed.body().links) {
            return Err(CustodyRefusal::HeldBundle);
        }
        held.insert(handle, signed);
    }

    let mut records = BTreeMap::new();
    let mut intents = Vec::with_capacity(imports.len());
    for (name, bundle) in imports {
        let intent = IntentHandle::new(name).map_err(|_| CustodyRefusal::ImportRecord)?;
        let bundle = IntentBundleHandle::new(bundle).map_err(|_| CustodyRefusal::ImportRecord)?;
        let entry = held
            .get(&bundle)
            .and_then(|signed| signed.body().contract(&intent))
            .ok_or(CustodyRefusal::ImportRecord)?;
        let contract =
            IntentContract::decode(&entry.contract).map_err(|_| CustodyRefusal::ImportRecord)?;
        if contract.to_artifact_bytes() != entry.contract
            || mint(&contract, services).ok().as_ref() != Some(&intent)
        {
            return Err(CustodyRefusal::ImportRecord);
        }
        let environment = CheckEnvironment::new().with_minted_intent_id(intent.as_str());
        if !contract.check(&environment).is_well_formed() {
            return Err(CustodyRefusal::ImportRecord);
        }
        let record = BundleRecord::parse(&entry.record, &intent)
            .map_err(|_| CustodyRefusal::ImportRecord)?;
        intents.push((
            intent.clone(),
            IntentRecord {
                contract,
                status: RegistryStatus::Proposed,
                supersedes: record.supersedes,
                superseded_by: None,
                acceptance: None,
            },
        ));
        records.insert(intent, bundle);
    }

    // Every adopted link is carried by a held bundle. The adopted links are bounded
    // (`validate_custody` checked `MAX_BUNDLE_LINKS`), so they are indexed once by their
    // encoding, and each held bundle's links — bounded by the held-bundle bytes charged
    // above — are looked up in that index.
    let mut uncarried: BTreeSet<Vec<u8>> = adopted.iter().map(|link| link.encode()).collect();
    for signed in held.values() {
        if uncarried.is_empty() {
            break;
        }
        for link in &signed.body().links {
            uncarried.remove(&link.encode());
        }
    }
    if !uncarried.is_empty() {
        return Err(CustodyRefusal::UncarriedLink);
    }

    Ok(RestoredImports {
        bundles: held.into_iter().collect(),
        records,
        intents,
    })
}

/// Hold the bundles and import records [`restore_imports`] validated again, and re-enter
/// every recorded contract into the registry at `proposed` (bn-3snfi): the restart's
/// counterpart of the writes `import_bundle` makes, and the registry's only other writer
/// beside it and `intent.lock`.
pub(crate) fn reenter_imports(state: &mut DaemonState, restored: RestoredImports) {
    state
        .signing_mut()
        .restore_bundles(restored.bundles, restored.records);
    for (handle, record) in restored.intents {
        state.put_intent(handle, record);
    }
}

/// Whether `successor` keeps every field `predecessor`'s policy protects, and does not
/// weaken the policy table itself. A field whose verb is anything but `unlocked` must be
/// byte-identical in the two artifacts: the directional verbs (`no-removal`,
/// `no-decrease`, …) need the RFC 0031 classifier, which has not shipped, so a change under
/// one is refused rather than guessed at.
///
/// The reviewer map must be *identical* (cr-crbbf2, RFC 0037 correction 22). It names who
/// may discharge a `review` verb, so changing it is a governance edit, not a field edit:
/// a revision that only swapped a principal would otherwise become the accepted head, and
/// the next reviewer-gated revision would be approved by the principal it injected. No
/// reviewed meta-governance amendment rule exists yet, so every difference is refused —
/// a dormant entry for a field whose verb is not `review`, a swap, an addition, a removal,
/// and a change that rides along with a policy tightening. An absent map and an empty one
/// decode to the same value and the same identity (ID2), so they are not a difference.
///
/// The other keys of the ID2 preimage are covered too: the fifteen protected groups above,
/// `policy` by the lattice order, and `schema_id`/`schema_epoch`, which decoding pins to
/// constants. `intent_id` and `name` are outside the preimage and govern nothing.
fn respects_predecessor(predecessor: &IntentContract, successor: &IntentContract) -> bool {
    if predecessor.policy_reviewers() != successor.policy_reviewers() {
        return false;
    }
    let before = predecessor.artifact_json();
    let after = successor.artifact_json();
    PolicyField::ALL.into_iter().all(|field| {
        let verb = predecessor.policy().verb(field);
        verb.is_weaker_or_equal(successor.policy().verb(field))
            && (verb == PolicyVerb::Unlocked
                || json_at(&before, field.contract_path())
                    == json_at(&after, field.contract_path()))
    })
}

/// The member of `document` a dotted contract path names, or `None` when it is absent.
fn json_at<'a>(document: &'a Json, path: &str) -> Option<&'a Json> {
    path.split('.').try_fold(document, |node, key| match node {
        Json::Object(fields) => fields.get(key),
        _ => None,
    })
}

/// The refusal for a revision that moves a field its predecessor's policy protects,
/// changes the reviewer map, or weakens the policy table: the one decision RFC 0031's unshipped classifier would have to
/// make, refused rather than guessed (the same refusal import gives, correction 19).
fn unclassified_revision() -> Fault {
    Fault::new(
        ErrorCode::IntentMutationDenied,
        "a revision changes a field its predecessor's policy protects, or its reviewers",
    )
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
    // This function only decodes the wire shape — there is no clock here, and no admitted
    // actor to compare against — so `accepted_by` and `timestamp` are read as presented,
    // held to the one spelling the protocol fixes so a record cannot carry a time nothing
    // can order. `accept` checks both against the call's admission before anything is
    // written (`require_admitted_locally`, RFC 0037 correction 23 extended, bn-342ek).
    Timestamp::new(&timestamp).map_err(|_| invalid())?;
    Ok(Acceptance {
        accepted_by,
        signature,
        timestamp,
        // Written by the daemon, never taken from the caller: plan §5.4 makes this the
        // record *the daemon* produced, and `rule audit.correlation` fixes its value.
        audit_record: audit.to_owned(),
        chain: Vec::new(),
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
        if !acceptance.chain.is_empty() {
            let chain = acceptance
                .chain
                .iter()
                .map(|element| {
                    Json::Object(BTreeMap::from([
                        ("scope".to_owned(), Json::String(element.scope.clone())),
                        (
                            "signature".to_owned(),
                            Json::String(element.signature.clone()),
                        ),
                        ("signer".to_owned(), Json::String(element.signer.clone())),
                    ]))
                })
                .collect();
            fields.insert("chain".to_owned(), Json::Array(chain));
        }
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

/// The refusal of an acceptance or a lock while the signing custody is unreconciled: the
/// signing state that would decide it may not survive a restart. Nothing changes.
fn unreconciled_custody() -> Fault {
    Fault::new(
        ErrorCode::UnsupportedSemanticFeature,
        "the signing custody is unreconciled; nothing was accepted or locked, and a restart \
         reconciles it",
    )
    .not_retryable()
}
