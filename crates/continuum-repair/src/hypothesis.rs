//! The hypothesis, the closed change kinds, and the proposal `repair.apply` carries.
//!
//! # The hypothesis is prose, and prose decides nothing
//!
//! `repair-transaction.schema.json` types `hypothesis` as a required string, and RFC
//! 0032 (correction 12) makes docs/41's sentence normative: the hypothesis "MUST NOT
//! contribute to any gate outcome, to the policy verdict, or to the receipt's
//! decision, and MUST NOT be interpolated into any typed field or into error text".
//! [`Hypothesis`] therefore has one reader, [`Hypothesis::as_prose`], for rendering to
//! a reviewer. Its [`fmt::Debug`] form states only a length, and no refusal in this
//! crate carries it, so the prose cannot reach an error message by accident.
//!
//! The consequence for INV-011: a hypothesis cannot weaken intent, because nothing
//! reads it. A hypothesis that *says* "relax the property" records that sentence and
//! changes no typed field (`tests/pr20_impl01_hypothesis_evidence.rs` pins this). The
//! weakening itself arrives, if at all, as a typed change, and that is where it is
//! refused ([`ChangeKind::Intent`], below) or classified (RFC 0031, gate 3, IMPL-04).
//!
//! # The change kinds are closed
//!
//! > the six change kinds (`rust`, `model`, `proof`, `correspondence`, `domain`,
//! > `intent`) […] are **closed**. […] A consumer that reads a gate name, gate status,
//! > gate profile, transaction status, or change kind it does not recognize MUST
//! > reject the artifact.
//! >
//! > — RFC 0032, "Versioning and revision"

use core::fmt;

/// The agent's hypothesis: untrusted prose, recorded beside the transaction and parsed
/// by nothing.
///
/// Any string is admissible, because the schema's type is `string` and no pattern or
/// length narrows it. The empty string is what a `draft` carries: `repair.begin` takes
/// no hypothesis (the IDL request is `{failure, gate_profile}`), yet the schema requires
/// the field at every status. [`Hypothesis::unstated`] names that value.
#[derive(Clone, PartialEq, Eq)]
pub struct Hypothesis(String);

impl Hypothesis {
    /// Record a hypothesis verbatim.
    #[must_use]
    pub fn new(prose: impl Into<String>) -> Self {
        Self(prose.into())
    }

    /// The hypothesis a `draft` carries before any `repair.apply`: the empty string.
    #[must_use]
    pub const fn unstated() -> Self {
        Self(String::new())
    }

    /// The prose, for rendering to a reviewer. Nothing in this crate calls it except the
    /// artifact writer, which copies it into the `hypothesis` field and nowhere else.
    #[must_use]
    pub fn as_prose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Hypothesis {
    /// The length only. RFC 0032 forbids interpolating the prose into error text, and a
    /// derived `Debug` would do exactly that through any `{:?}` of a containing type.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hypothesis(<{} bytes of untrusted prose>)", self.0.len())
    }
}

/// The closed set of change kinds (`repair-transaction.schema.json` `changes[].kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangeKind {
    /// Program source.
    Rust,
    /// The mathematical model.
    Model,
    /// A proof.
    Proof,
    /// A model–program correspondence.
    Correspondence,
    /// A domain pack.
    Domain,
    /// The Intent Contract itself. Admissible only on a reclassified transaction
    /// (RFC 0032, "Intent integrity and reclassification").
    Intent,
}

impl ChangeKind {
    /// Every kind, in the schema's enum order.
    pub const ALL: [Self; 6] = [
        Self::Rust,
        Self::Model,
        Self::Proof,
        Self::Correspondence,
        Self::Domain,
        Self::Intent,
    ];

    /// The schema token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Model => "model",
            Self::Proof => "proof",
            Self::Correspondence => "correspondence",
            Self::Domain => "domain",
            Self::Intent => "intent",
        }
    }

    /// The kind a schema token names, or `None` for any other token. A reader MUST
    /// reject an unrecognized kind, never map it to a known one.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.token() == token)
    }
}

/// One entry of `changes`: a kind and a digest.
///
/// The digest is carried opaque. Its derivation and normalization are patch identity,
/// PR-20 / IMPL-02; the schema types it as a bare string, so nothing here narrows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    kind: ChangeKind,
    digest: String,
}

impl Change {
    /// A change of `kind` whose content has `digest`.
    #[must_use]
    pub fn new(kind: ChangeKind, digest: impl Into<String>) -> Self {
        Self {
            kind,
            digest: digest.into(),
        }
    }

    /// The change kind.
    #[must_use]
    pub const fn kind(&self) -> ChangeKind {
        self.kind
    }

    /// The opaque content digest.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// What `repair.apply` carries besides the transaction handle: the typed changes and
/// the hypothesis (IDL `repair.apply` request `{repair, changes, hypothesis}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    hypothesis: Hypothesis,
    changes: Vec<Change>,
}

impl Proposal {
    /// A proposal. Admission is [`crate::transaction::RepairTransaction::apply`]'s, not
    /// this constructor's: whether an `intent` change is admissible depends on the
    /// transaction it is applied to.
    #[must_use]
    pub const fn new(hypothesis: Hypothesis, changes: Vec<Change>) -> Self {
        Self {
            hypothesis,
            changes,
        }
    }

    /// The hypothesis.
    #[must_use]
    pub const fn hypothesis(&self) -> &Hypothesis {
        &self.hypothesis
    }

    /// The changes, in the order proposed.
    #[must_use]
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_renders_the_prose() {
        let hypothesis = Hypothesis::new("SECRET-PROSE relax the property");
        let rendered = format!("{hypothesis:?}");
        assert!(!rendered.contains("SECRET-PROSE"), "{rendered}");
        assert!(rendered.contains("31 bytes"), "{rendered}");
    }

    #[test]
    fn change_kind_tokens_round_trip_and_reject_unknowns() {
        for kind in ChangeKind::ALL {
            assert_eq!(ChangeKind::from_token(kind.token()), Some(kind));
        }
        assert_eq!(ChangeKind::from_token("Intent"), None);
        assert_eq!(ChangeKind::from_token("config"), None);
    }
}
