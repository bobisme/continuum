//! `Published<H>` name comparison, differential (bn-283p6, INV-017).
//!
//! `Published::attest` ties a name to a publication receipt only when the name spells the
//! receipt's handle. The daemon supplies that comparison for its own handle types
//! (`continuumd::daemon::identity`, `PublishedName` for `WorkspaceHandle`,
//! `ContinuationHandle`, and `Commitment`). The store supplies it for its own
//! `ArtifactHandle`: parse the spelling, compare the handles.
//!
//! The two are independent implementations of one relation, so this file runs both over
//! the same receipts and the same candidate spellings and requires one verdict:
//!
//! - **oracle**: `continuum_workspace` — `ArtifactHandle::from_str`, then
//!   `Published::<continuum_workspace::artifact_path::ArtifactHandle>::attest`;
//! - **subject**: `continuumd` — the wire handle's constructor, then `Published::attest`
//!   through the daemon's `PublishedName` impl.
//!
//! A candidate a wire type refuses to construct is skipped for that subject. `Commitment`
//! admits every spelling, so every candidate reaches at least one subject.

use std::str::FromStr;
use std::sync::Arc;

use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{
    ActorId, AuditLog, AuthorityLevel, CapabilityDescriptor, CapabilityToken, ContentIdentifier,
    IdentityUnavailable, PublicationReceipt, Published, PublishedName, ReferenceStore,
};
use continuumd::protocol::scalar::{Commitment, ContinuationHandle, WorkspaceHandle};

/// Names content by its own text, so the test chooses every published handle.
struct Verbatim;

impl ContentIdentifier for Verbatim {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        let text = std::str::from_utf8(content).map_err(|_| IdentityUnavailable)?;
        ArtifactHandle::new(class, text).map_err(|_| IdentityUnavailable)
    }
}

fn receipts() -> Vec<PublicationReceipt> {
    let author = CapabilityToken::mint("author").expect("a capability token");
    let store = ReferenceStore::builder(Verbatim, Arc::new(AuditLog::new()))
        .capability(CapabilityDescriptor::new(
            author.clone(),
            ActorId::new("author"),
            AuthorityLevel::Propose,
        ))
        .build();
    [
        (ArtifactClass::WorkspaceSnapshot, "abc123"),
        (ArtifactClass::WorkspaceSnapshot, "abc124"),
        (ArtifactClass::Continuation, "abc123"),
        (ArtifactClass::Task, "abc123"),
        (ArtifactClass::Task, "d_e-f"),
    ]
    .into_iter()
    .map(|(class, identity)| {
        store
            .publish(class, identity.as_bytes().to_vec(), &author)
            .expect("the author publishes")
    })
    .collect()
}

/// Every spelling the receipts could be confused with: each receipt's own, the same
/// identities under every other prefix, near misses, and malformed text.
fn candidates(receipts: &[PublicationReceipt]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for receipt in receipts {
        let own = receipt.handle().to_string();
        let identity = own
            .strip_prefix(receipt.handle().class().prefix())
            .expect("a handle spells its prefix")
            .to_owned();
        out.push(own.clone());
        for prefix in ["ws_", "cont_", "task_", "evid_"] {
            out.push(format!("{prefix}{identity}"));
        }
        out.push(format!("{own}x"));
        out.push(own.to_uppercase());
    }
    out.extend(["ws_", "cont_", "task_", "", "ws_a b"].map(str::to_owned));
    out.sort();
    out.dedup();
    out
}

/// The oracle's verdict: the store parses the spelling and compares handles.
fn oracle(receipt: &PublicationReceipt, text: &str) -> bool {
    ArtifactHandle::from_str(text)
        .is_ok_and(|parsed| Published::<ArtifactHandle>::attest(receipt, parsed).is_ok())
}

/// The subject's verdict for one daemon handle type, or [`None`] when the wire type does
/// not admit the spelling.
fn subject<H: PublishedName>(
    receipt: &PublicationReceipt,
    text: &str,
    build: impl Fn(&str) -> Option<H>,
) -> Option<bool> {
    build(text).map(|name| Published::attest(receipt, name).is_ok())
}

/// **Differential: the daemon's `PublishedName` impls agree with the store's own handle
/// comparison on every receipt and every candidate spelling.**
#[test]
fn the_daemon_and_the_store_attest_the_same_names() {
    let receipts = receipts();
    let candidates = candidates(&receipts);
    let mut compared = 0_usize;
    let mut attested = 0_usize;
    for receipt in &receipts {
        for text in &candidates {
            let expected = oracle(receipt, text);
            attested += usize::from(expected);
            let verdicts = [
                subject(receipt, text, |t| WorkspaceHandle::new(t).ok()),
                subject(receipt, text, |t| ContinuationHandle::new(t).ok()),
                subject(receipt, text, |t| Some(Commitment::new(t))),
            ];
            for verdict in verdicts.into_iter().flatten() {
                assert_eq!(
                    verdict,
                    expected,
                    "the daemon and the store disagree on `{text}` against `{}`",
                    receipt.handle()
                );
                compared += 1;
            }
        }
    }
    assert_eq!(
        attested,
        receipts.len(),
        "each receipt attests exactly its own spelling, so the comparison is not vacuous"
    );
    assert!(
        compared > candidates.len() * receipts.len(),
        "most candidates reach at least two subjects ({compared})"
    );
}
