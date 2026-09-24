//! The `intent` namespace: request and response structs for its 8 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `intent.get`.
    struct IntentGetRequest {
        /// IDL `intent: IntentHandle required`.
        intent: IntentHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `intent.get`.
    struct IntentGetResponse {
        /// IDL `intent: IntentHandle required`.
        intent: IntentHandle required;
        /// IDL `contract: Opaque required`.
        contract: Opaque required;
        /// Registry record, including protection status and acceptance
        /// chain (`schemas/intent-registry-record.schema.json`).
        record: Opaque required;
    }
}

protocol_struct! {
    /// The `request` body of `intent.diff`.
    struct IntentDiffRequest {
        /// IDL `before: IntentHandle required`.
        before: IntentHandle required;
        /// IDL `after: IntentHandle required`.
        after: IntentHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `intent.diff`.
    struct IntentDiffResponse {
        /// IDL `diff: DiffHandle required`.
        diff: DiffHandle required;
        /// IDL `summary: Opaque required`.
        summary: Opaque required;
    }
}

protocol_struct! {
    /// The `request` body of `intent.propose_revision`.
    struct IntentProposeRevisionRequest {
        /// IDL `base: IntentHandle required`.
        base: IntentHandle required;
        /// IDL `changes: IntentChangeSet required`.
        changes: IntentChangeSet required;
    }
}

protocol_struct! {
    /// The `response` body of `intent.propose_revision`.
    struct IntentProposeRevisionResponse {
        /// IDL `proposal: IntentHandle required`.
        proposal: IntentHandle required;
        /// IDL `diff: DiffHandle required`.
        diff: DiffHandle required;
    }
}

protocol_struct! {
    /// The `request` body of `intent.accept`.
    struct IntentAcceptRequest {
        /// IDL `proposal: IntentHandle required`.
        proposal: IntentHandle required;
        /// Acceptance record, including the signature chain to verify
        /// (plan §4.2.1).
        acceptance: Opaque required;
        /// The signed intent bundle the acceptance record came from, when
        /// the proposal was imported rather than authored locally. CI fails
        /// closed with `AcceptanceChainInvalid` when the referenced bundle
        /// is absent or its chain does not verify (plan §4.2.1).
        bundle: IntentBundleHandle optional;
    }
}

protocol_struct! {
    /// The `response` body of `intent.accept`.
    struct IntentAcceptResponse {
        /// IDL `intent: IntentHandle required`.
        intent: IntentHandle required;
        /// IDL `record: Opaque required`.
        record: Opaque required;
    }
}

protocol_struct! {
    /// The `request` body of `intent.reject`.
    struct IntentRejectRequest {
        /// IDL `proposal: IntentHandle required`.
        proposal: IntentHandle required;
        /// IDL `reason: String required`.
        reason: String required;
    }
}

protocol_struct! {
    /// The `response` body of `intent.reject`.
    struct IntentRejectResponse {
        /// IDL `proposal: IntentHandle required`.
        proposal: IntentHandle required;
    }
}

protocol_struct! {
    /// The `request` body of `intent.lock`.
    struct IntentLockRequest {
        /// IDL `intent: IntentHandle required`.
        intent: IntentHandle required;
        /// Field-to-verb policy table
        /// (`schemas/intent-contract.schema.json` `policy_verb`).
        policy: map<String,String> required;
    }
}

protocol_struct! {
    /// The `response` body of `intent.lock`.
    struct IntentLockResponse {
        /// IDL `intent: IntentHandle required`.
        intent: IntentHandle required;
        /// IDL `policy: map<String,String> required`.
        policy: map<String,String> required;
    }
}

protocol_struct! {
    /// The `request` body of `intent.export_bundle` (protocol 3.8).
    struct IntentExportBundleRequest {
        /// The contracts to export. Non-empty, at most the bundle bound.
        intents: list<IntentHandle> required;
    }
}

protocol_struct! {
    /// The `response` body of `intent.export_bundle` (protocol 3.8).
    struct IntentExportBundleResponse {
        /// The signed bundle's identity. The daemon holds it.
        bundle: IntentBundleHandle required;
        /// The signed bundle's bytes (`rule intent.bundles`).
        content: Bytes required;
    }
}

protocol_struct! {
    /// The `request` body of `intent.import_bundle` (protocol 3.8).
    struct IntentImportBundleRequest {
        /// A signed bundle's bytes (`rule intent.bundles`).
        content: Bytes required;
    }
}

protocol_struct! {
    /// The `response` body of `intent.import_bundle` (protocol 3.8).
    struct IntentImportBundleResponse {
        /// The bundle's identity. The daemon holds it.
        bundle: IntentBundleHandle required;
        /// The bundle signature's typed provenance under local policy.
        outcome: SignatureOutcome required;
        /// The contracts this import entered into the registry, at `proposed`.
        imported: list<IntentHandle> required;
        /// The standing facts the import adopted into the signing registry.
        adopted: U32 required;
    }
}
