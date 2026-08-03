//! Content identity for evidence nodes and edges, and the `ev_` handle that names one
//! (docs/44 "Graph model", ADR-0013, ADR-0037).
//!
//! # Two things, deliberately separated
//!
//! > ```text
//! > struct EvidenceNode {
//! >     id: ContentId,
//! >     …
//! > }
//! > ```
//! >
//! > — docs/44, "Graph model"
//!
//! > in certified lanes, content identity is exact canonical identity; hash collisions are
//! > resolved by canonical comparison; 256-bit-hash identity is permitted only in
//! > explicitly labeled non-certified modes (ADR-0013)
//! >
//! > — plan §20, dependency rules
//!
//! Those two sentences together fix the shape. An evidence artifact's identity **is** the
//! canonical encoding of what it is — [`EvidenceIdentity`], a thin wrapper over
//! `continuum-value`'s [`ContentIdentity`], whose equality and ordering are byte order on
//! that encoding and consult no hasher. The short `ev_…` token the schemas carry
//! ([`EvidenceHandle`], `^ev_[A-Za-z0-9_-]+$`) is a **name for** that identity, produced by
//! a 256-bit digest, and ADR-0013 permits that only in an explicitly labeled non-certified
//! mode — so [`DigestNaming`] computes a `HashIdentity` under the label
//! [`HANDLE_LABEL`] and nothing in this crate ever decides that two artifacts are the same
//! by comparing handles.
//!
//! The consequence worth stating: a collision between two handles is a naming accident that
//! this crate can *detect* (the identities differ) and never a silent merge. `EvidenceGraph`
//! keys on the identity, not the handle.
//!
//! # The question this split was built for, and the answer it received
//!
//! > **D2 — `edge_id` is the content identity of what the edge asserts, named through one
//! > seam.** […] The wire's naming is the deployment's `ContentIdentifier` seam at
//! > `ArtifactClass::Evidence` — the same seam and the same class that names nodes, so one
//! > identity kernel names every evidence artifact.
//! >
//! > — RFC 0038, "Edges: what a `CHECKED_BY` edge names" (bn-3sypm, F14)
//!
//! The split is what made that answer cheap. This crate commits to the *preimage*: the
//! canonical encoding of the record, which is a statement about what an artifact **is** and
//! is the same statement under any naming function. It does not commit to the preimage →
//! `ev_…` token function, which is [`EvidenceNaming`] — a seam, with one implementation
//! supplied here for use and tests. RFC 0038 filled that seam with the daemon's identity
//! kernel, and no identity, edge, or graph type moved.
//!
//! # The library's identity is not the wire's name (RFC 0038 D4)
//!
//! Two derivations existed for a *node*, and RFC 0038 chose between them:
//!
//! | | preimage | consequence |
//! |---|---|---|
//! | `continuumd` (**normative on the wire**) | the referenced artifact commitment and the capture profile | two ingests of one trace under one profile converge, in any process, by any producer, at any time |
//! | this crate | the whole record minus status and idempotency key — which includes [`crate::provenance::Provenance`], and so the actor and the wall clock | two producers of the identical observation are two records |
//!
//! The daemon's won, on this RFC's own sentence: "a replayed write returns the original node
//! identity" is a claim *across* processes and producers, and an identity containing a clock
//! cannot support it. So [`EvidenceIdentity`] is a **within-graph content key** — what
//! [`crate::graph::EvidenceGraph`] files records under and decides sameness by — and it is
//! deliberately never rendered as a handle: [`fmt::Display`] is hexadecimal, and
//! [`crate::node::EvidenceNode::to_record`] and [`crate::edge::EvidenceEdge::to_record`]
//! take the `ev_` handle as a **parameter**. A graph joined to the wire therefore *supplies*
//! the wire's names rather than deriving them, which is the reconciliation: the two
//! derivations do not compete, because only one of them ever reaches a schema instance.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | identity is the canonical encoding, not a digest | ADR-0013, plan §20 | `identity_equality_never_consults_a_hash` |
//! | a digest name is a labeled non-certified index | ADR-0013 | `a_handle_is_a_labeled_non_certified_name` |
//! | handles match `^ev_[A-Za-z0-9_-]+$` | both schemas | `handles_match_the_schema_pattern` |
//! | naming is a pure function of the identity | INV-005, INV-006 | `naming_is_deterministic_across_calls` |
//! | the naming function is replaceable | RFC 0038 D2 (bn-3sypm) | `a_second_naming_scheme_renames_without_re_identifying` |
//! | the wire's name is supplied, not derived | RFC 0038 D4 (bn-3sypm) | `a_records_wire_name_is_a_parameter_not_a_derivation` |

use core::fmt;

use continuum_value::identity::{
    Blake3Hasher, ContentHasher, ContentIdentity, Digest256, HashIdentity, NonCertifiedLabel,
};
use continuum_value::value::Value;

/// The ADR-0013 non-certified label every `ev_` handle is minted under.
///
/// ADR-0013 permits 256-bit-hash identity "only in explicitly labeled non-certified modes".
/// A handle is such a mode: it is how an evidence artifact is *referred to* in a schema
/// instance, a path, or a log line, and it is never how this crate decides two artifacts are
/// the same.
pub const HANDLE_LABEL: &str = "evidence-graph-handle";

/// The class prefix the evidence schemas give every node and edge handle.
pub const HANDLE_PREFIX: &str = "ev_";

/// The exact identity of an evidence node or edge.
///
/// Equality and ordering are byte order on the canonical encoding of the record, so a
/// `BTreeMap` keyed by one iterates in a deterministic, content-derived order and two
/// records are the same artifact exactly when they encode identically. There is no `Hash`
/// impl, mirroring [`ContentIdentity`]: a hash map keyed by an identity is an affordance for
/// deciding sameness by digest, which is what ADR-0013 removes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EvidenceIdentity(ContentIdentity);

impl EvidenceIdentity {
    /// The identity of a canonical record.
    ///
    /// Infallible, because every [`Value`] has a canonical encoding.
    #[must_use]
    pub fn of(record: &Value) -> Self {
        Self(ContentIdentity::of(record))
    }

    /// The canonical bytes this identity *is*.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        self.0.canonical_bytes()
    }

    /// The underlying ADR-0013 content identity.
    #[must_use]
    pub const fn content_identity(&self) -> &ContentIdentity {
        &self.0
    }

    /// The 256-bit digest of this identity under `H` — an index, never an identity.
    #[must_use]
    pub fn digest<H: ContentHasher>(&self) -> Digest256 {
        self.0.digest::<H>()
    }
}

impl fmt::Display for EvidenceIdentity {
    /// The hexadecimal canonical encoding.
    ///
    /// Deliberately not the `ev_` handle: rendering an identity as its name would make the
    /// two interchangeable in a log, and they are not.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// An `ev_` handle: the token the evidence schemas carry for a node or an edge.
///
/// > `"node_id": { "pattern": "^ev_[A-Za-z0-9_-]+$" }`
/// > `"edge_id": { "pattern": "^ev_[A-Za-z0-9_-]+$" }`
/// >
/// > — `evidence-graph-node.schema.json`, `evidence-graph-edge.schema.json`
///
/// One pattern for both, which is why one type serves both.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceHandle(String);

impl EvidenceHandle {
    /// The pattern this type accepts, in the schemas' own spelling.
    pub const PATTERN: &'static str = "^ev_[A-Za-z0-9_-]+$";

    /// Parse a handle.
    ///
    /// # Errors
    ///
    /// [`HandleError`] when the `ev_` prefix is missing, or when the identity half is empty
    /// or carries a character outside `[A-Za-z0-9_-]`.
    pub fn new(text: &str) -> Result<Self, HandleError> {
        let identity = text
            .strip_prefix(HANDLE_PREFIX)
            .ok_or(HandleError::Prefix)?;
        if identity.is_empty() {
            return Err(HandleError::Empty);
        }
        if !identity
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(HandleError::Character);
        }
        Ok(Self(text.to_owned()))
    }

    /// The canonical spelling, including the `ev_` prefix.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The identity half, after the `ev_` prefix.
    #[must_use]
    pub fn identity_token(&self) -> &str {
        self.0
            .strip_prefix(HANDLE_PREFIX)
            .expect("a constructed handle carries the prefix")
    }
}

impl fmt::Display for EvidenceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a handle was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HandleError {
    /// The text does not begin `ev_`.
    Prefix,
    /// The identity half is empty.
    Empty,
    /// The identity half carries a character outside `[A-Za-z0-9_-]`.
    Character,
}

impl fmt::Display for HandleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Prefix => "an evidence handle begins `ev_`",
            Self::Empty => "the identity half of an evidence handle is empty",
            Self::Character => "the identity half carries a character outside [A-Za-z0-9_-]",
        };
        f.write_str(message)
    }
}

impl core::error::Error for HandleError {}

/// How an [`EvidenceIdentity`] is given an `ev_` name.
///
/// A seam rather than a function, because the wire's naming is a deployment's identity
/// kernel and not this crate's digest: RFC 0038 D2 fixes `edge_id` as that kernel's answer
/// over the edge's canonical preimage, at `ArtifactClass::Evidence`, so one seam names every
/// evidence artifact. An implementation must be **pure**: two calls with equal identities,
/// in any process, return equal handles (INV-005, INV-006) — the same contract
/// `continuum-workspace`'s `ContentIdentifier` states for artifact publication.
pub trait EvidenceNaming {
    /// Name an identity.
    fn name(&self, identity: &EvidenceIdentity) -> EvidenceHandle;
}

/// The naming this crate supplies: `ev_` followed by the identity's digest under `H`.
///
/// ADR-0013-shaped, and the type says so: the handle is computed through
/// [`HashIdentity`] under [`HANDLE_LABEL`], which is the ADR's "explicitly labeled
/// non-certified mode", and the label travels with the value rather than living in a
/// comment. `DigestNaming<Blake3Hasher>` is the default because BLAKE3 is the workspace's
/// one declared digest (`tools/governance/dependency-rationale.toml`, GOV-1-07).
///
/// This is **not** the wire's naming. RFC 0038 D2 put that in the deployment's
/// `ContentIdentifier` seam, so this implementation is what a library-only caller gets:
/// available, deterministic, and replaceable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DigestNaming<H: ContentHasher> {
    hasher: core::marker::PhantomData<H>,
}

impl<H: ContentHasher> DigestNaming<H> {
    /// The naming.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            hasher: core::marker::PhantomData,
        }
    }

    /// The labeled non-certified identity behind a handle, for a caller that wants to see
    /// the label and the algorithm rather than the token alone.
    ///
    /// # Panics
    ///
    /// Never: [`HANDLE_LABEL`] is a compile-time constant inside `NonCertifiedLabel`'s
    /// character class, and `a_handle_is_a_labeled_non_certified_name` pins that it parses.
    #[must_use]
    pub fn hash_identity(identity: &EvidenceIdentity) -> HashIdentity {
        let label = NonCertifiedLabel::new(HANDLE_LABEL)
            .expect("the handle label is inside the non-certified label character class");
        // The value hashed is the identity's own canonical bytes, so the name is a function
        // of the identity and of nothing else.
        HashIdentity::compute::<H>(&Value::bytes(identity.canonical_bytes().to_vec()), label)
    }
}

impl<H: ContentHasher> EvidenceNaming for DigestNaming<H> {
    /// # Panics
    ///
    /// Never: a hexadecimal digest token is inside `^ev_[A-Za-z0-9_-]+$`.
    fn name(&self, identity: &EvidenceIdentity) -> EvidenceHandle {
        let token = Self::hash_identity(identity).digest().to_token();
        EvidenceHandle::new(&format!("{HANDLE_PREFIX}{token}"))
            .expect("a hexadecimal digest token is inside the handle character class")
    }
}

/// The naming a caller gets when it names none: BLAKE3 under [`HANDLE_LABEL`].
pub type DefaultNaming = DigestNaming<Blake3Hasher>;

#[cfg(test)]
mod tests {
    use continuum_value::identity::{Fnv1aPlaceholder, IdentityMode};

    use super::*;
    use crate::actor::field;

    fn record(text: &str) -> Value {
        Value::record([(field("claim"), Value::text(text))]).expect("one field")
    }

    #[test]
    fn identity_equality_never_consults_a_hash() {
        // ADR-0013: certified identity is the canonical encoding. Two hashers give the
        // same identity and two different names, so a name change is never an identity
        // change.
        let identity = EvidenceIdentity::of(&record("a"));
        let blake = DigestNaming::<Blake3Hasher>::new().name(&identity);
        let placeholder = DigestNaming::<Fnv1aPlaceholder>::new().name(&identity);
        assert_ne!(blake, placeholder);
        assert_eq!(identity, EvidenceIdentity::of(&record("a")));
        assert_eq!(
            identity.canonical_bytes(),
            EvidenceIdentity::of(&record("a")).canonical_bytes()
        );
    }

    #[test]
    fn distinct_records_have_distinct_identities() {
        let left = EvidenceIdentity::of(&record("a"));
        let right = EvidenceIdentity::of(&record("b"));
        assert_ne!(left, right);
        assert_ne!(left.canonical_bytes(), right.canonical_bytes());
        // …and the identity is exactly the record's canonical encoding.
        assert_eq!(left.canonical_bytes(), record("a").encode());
    }

    #[test]
    fn identity_order_is_byte_order_on_the_encoding() {
        let mut identities: Vec<EvidenceIdentity> = ["c", "a", "b"]
            .into_iter()
            .map(|text| EvidenceIdentity::of(&record(text)))
            .collect();
        identities.sort();
        let encodings: Vec<Vec<u8>> = identities
            .iter()
            .map(|identity| identity.canonical_bytes().to_vec())
            .collect();
        let mut sorted = encodings.clone();
        sorted.sort();
        assert_eq!(encodings, sorted);
    }

    #[test]
    fn a_handle_is_a_labeled_non_certified_name() {
        let identity = EvidenceIdentity::of(&record("a"));
        let hash = DigestNaming::<Blake3Hasher>::hash_identity(&identity);
        assert_eq!(hash.label().as_str(), HANDLE_LABEL);
        assert!(hash.is_cryptographic());
        // The digest is the token's identity half, so the label and the name agree.
        let handle = DigestNaming::<Blake3Hasher>::new().name(&identity);
        assert_eq!(handle.identity_token(), hash.digest().to_token());
        // ADR-0013's own reading of what a hash name is.
        assert_eq!(
            continuum_value::identity::Identity::non_certified::<Blake3Hasher>(
                &Value::bytes(identity.canonical_bytes().to_vec()),
                NonCertifiedLabel::new(HANDLE_LABEL).expect("well formed"),
            )
            .mode(),
            IdentityMode::NonCertifiedHash256
        );
    }

    #[test]
    fn handles_match_the_schema_pattern() {
        assert_eq!(EvidenceHandle::PATTERN, "^ev_[A-Za-z0-9_-]+$");
        for text in ["ev_a", "ev_1a-2b_3c", "ev_A"] {
            assert_eq!(
                EvidenceHandle::new(text).expect("well formed").as_str(),
                text
            );
        }
        assert_eq!(EvidenceHandle::new("node_a"), Err(HandleError::Prefix));
        assert_eq!(EvidenceHandle::new("ev_"), Err(HandleError::Empty));
        assert_eq!(EvidenceHandle::new("ev_a/b"), Err(HandleError::Character));
        assert_eq!(EvidenceHandle::new(""), Err(HandleError::Prefix));
        let handle = EvidenceHandle::new("ev_abc").expect("well formed");
        assert_eq!(handle.identity_token(), "abc");
        assert_eq!(handle.to_string(), "ev_abc");
    }

    #[test]
    fn a_named_handle_is_always_well_formed() {
        // The naming's `expect` is discharged by construction, over a range of records.
        for index in 0..64u32 {
            let identity = EvidenceIdentity::of(&record(&index.to_string()));
            let handle = DefaultNaming::new().name(&identity);
            assert_eq!(
                EvidenceHandle::new(handle.as_str()).expect("well formed"),
                handle
            );
            assert_eq!(handle.identity_token().len(), 64);
        }
    }

    #[test]
    fn naming_is_deterministic_across_calls() {
        // INV-005/INV-006: naming is a pure function of the identity.
        let identity = EvidenceIdentity::of(&record("a"));
        let naming = DefaultNaming::new();
        assert_eq!(naming.name(&identity), naming.name(&identity));
        assert_eq!(naming.name(&identity), DefaultNaming::new().name(&identity));
    }

    #[test]
    fn a_records_wire_name_is_a_parameter_not_a_derivation() {
        // RFC 0038 D4. Two nodes that say the same thing about the same artifact but were
        // written by two producers have two *library* identities, because provenance is in
        // this crate's preimage. The wire's identity excludes provenance, so on the wire
        // they are one node — and that disagreement is harmless for exactly one reason:
        // nothing here renders an identity as a handle. `to_record` takes the handle.
        use crate::node::{ClaimId, EvidenceNode, IdempotencyKey, NodeKind};
        use crate::provenance::{ArtifactRef, Provenance, Timestamp};

        let propose = |actor: &str, at: &str| {
            EvidenceNode::propose(
                NodeKind::Run,
                ArtifactRef::new("ev_trace9f").expect("well formed"),
                ClaimId::new("claim-1").expect("non-empty"),
                IdempotencyKey::new("key-1").expect("non-empty"),
                Provenance::new(
                    crate::actor::ActorId::new(actor).expect("well formed"),
                    Timestamp::new(at).expect("well formed"),
                    [],
                ),
            )
        };
        let first = propose("agent:swarm-1", "2026-08-01T12:00:00.000Z");
        let second = propose("agent:swarm-2", "2026-08-01T12:00:01.000Z");
        assert_ne!(
            first.identity(),
            second.identity(),
            "provenance is inside this crate's content key"
        );

        // The wire's name is whatever the daemon's identity kernel said, and both records
        // render under it without either one deriving it. That is the join RFC 0038 D4
        // names: the library accepts the wire's name as input.
        let wire = EvidenceHandle::new("ev_daemonderived").expect("well formed");
        let rendered = |node: &EvidenceNode| match node.to_record(&wire) {
            Value::Record(fields) => fields
                .get(&crate::actor::field("node_id"))
                .cloned()
                .expect("the schema's node_id"),
            other => panic!("expected a record, got {other:?}"),
        };
        assert_eq!(rendered(&first), Value::text(wire.as_str()));
        assert_eq!(rendered(&second), Value::text(wire.as_str()));

        // …and the identity is never spelled as a handle: `Display` is hexadecimal, so a
        // log line cannot be mistaken for a wire name.
        assert!(!first.identity().to_string().starts_with(HANDLE_PREFIX));
    }

    #[test]
    fn a_second_naming_scheme_renames_without_re_identifying() {
        // The F14 seam: RFC 0038 D2's decision is a new `EvidenceNaming`, not a new
        // identity.
        struct Ordinal;
        impl EvidenceNaming for Ordinal {
            fn name(&self, identity: &EvidenceIdentity) -> EvidenceHandle {
                let token: String = identity
                    .canonical_bytes()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                EvidenceHandle::new(&format!("ev_{token}")).expect("hexadecimal")
            }
        }

        let identity = EvidenceIdentity::of(&record("a"));
        let default = DefaultNaming::new().name(&identity);
        let ordinal = Ordinal.name(&identity);
        assert_ne!(default, ordinal);
        // One identity, two names, and the identity is what the graph keys on.
        assert_eq!(identity, EvidenceIdentity::of(&record("a")));
    }
}
