//! Seal: freezing a workspace in progress into the artifact everything else names
//! (plan §4.2, §4.4, PR 3 / IMPL-02).
//!
//! # What this module decides
//!
//! > 4. Snapshot manifest is canonicalized and hashed. 5. Intent is attached by identity,
//! > not copied implicitly. 6. Snapshot is sealed and immutable.
//! >
//! > — `notes/plan/docs/35_CONTINUUMD_WORKBENCH_DAEMON.md`, "Snapshot transaction"
//!
//! Importing, overlaying, and forking all produce values that exist only in the process
//! that made them. Sealing is the step that makes one *durable and addressable*: every
//! record the descriptor is made of goes to a [`ReferenceStore`], children before parents,
//! and what comes back is a [`SealedWorkspace`] — a snapshot identity, a descriptor
//! identity, and the receipts proving both were published.
//!
//! That value is what PR 5's daemon hands to a client: a name for content it can fetch,
//! not a handle onto a live process's memory.
//!
//! # Ordering is the whole correctness argument
//!
//! [`WorkspaceDescriptor::records`] yields records children before parents, and this module
//! publishes them in exactly that order without reordering, filtering, or deduplicating.
//! A parent record names its children by identity, so publishing a parent first would make
//! a reachable artifact point at content the store does not hold — the dangling reference
//! docs/35 rules out for the index, one level up. Nothing here needs to *decide* the
//! order; it needs to not destroy it.
//!
//! # Convergence is the store's, not this module's
//!
//! Sealing two identical workspaces publishes the same records twice, and the second pass
//! converges: the store derives each identity from the bytes it is given, finds the
//! identity already present, and returns a receipt without a second index entry
//! (`publication.rs`, "a publication of content the store already holds succeeds"). So
//! structural sharing between two workspaces — a shared subtree, a shared lockfile — costs
//! nothing extra at seal time, and this module contains no cache, no "already published"
//! set, and no second notion of what the store holds. A dedup this module could observe
//! would be an existence oracle (plan §4.5); one the store performs is not.
//!
//! # The identity seam has to be the same one
//!
//! A descriptor's identities were derived through the [`ContentIdentifier`] its snapshot
//! was built with; the store derives them again, through *its own*. If those two disagree,
//! the store holds the right bytes under a name the descriptor does not use, and every
//! later fetch by descriptor identity misses. That is [`SealError::IdentityDisagreement`],
//! checked per record and refused loudly — never patched up by trusting the receipt, which
//! would silently rewrite the artifact's name.
//!
//! # What sealing does not do
//!
//! - **It attaches no intent.** docs/35's step 5 attaches the governing Intent Contract by
//!   identity; `in_*` handles and the intent registry are PR 4's, and
//!   [`components`](crate::components) deliberately does not carry the reference yet.
//!   Sealing a workspace today seals its files and its components, and nothing about the
//!   contract that governs it.
//! - **It publishes no `workspace-snapshot` document.** What reaches the store is the
//!   crate's own record grammar — the Merkle nodes and the component records —
//!   not the JSON artifact `notes/plan/schemas/workspace-snapshot.schema.json` describes.
//!   Assembling that document needs the epochs and the intent reference, which are two
//!   other PRs' values.
//! - **It does not re-derive anything.** The descriptor arrives already named. Sealing
//!   moves bytes; it never changes an identity.
//!
//! # Example
//!
//! ```
//! use continuum_workspace::components::WorkspaceDescriptor;
//! use continuum_workspace::publication::{
//!     ActorId, AuditLog, AuthorityLevel, CapabilityDescriptor, CapabilityToken, ReferenceStore,
//! };
//! use continuum_workspace::seal::SealedWorkspace;
//! use continuum_workspace::snapshot::{Snapshot, WorkspaceContent, WorkspacePath};
//! # use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
//! # use continuum_workspace::publication::{ContentIdentifier, IdentityUnavailable};
//! # #[derive(Clone)]
//! # struct HexIdentity;
//! # impl ContentIdentifier for HexIdentity {
//! #     fn identify(&self, class: ArtifactClass, content: &[u8])
//! #         -> Result<ArtifactHandle, IdentityUnavailable> {
//! #         let mut token = String::with_capacity(content.len() * 2);
//! #         for byte in content {
//! #             token.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
//! #             token.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
//! #         }
//! #         ArtifactHandle::new(class, &token).map_err(|_| IdentityUnavailable)
//! #     }
//! # }
//! let mut content = WorkspaceContent::new();
//! content.insert(WorkspacePath::new("src/lib.rs")?, b"fn main() {}".to_vec())?;
//! let descriptor = WorkspaceDescriptor::builder()
//!     .dependencies(b"[[package]]".to_vec())
//!     .build(Snapshot::build(&content, &HexIdentity)?, &HexIdentity)?;
//!
//! let capability = CapabilityToken::mint("publisher")?;
//! let store = ReferenceStore::builder(HexIdentity, AuditLog::new())
//!     .capability(CapabilityDescriptor::new(
//!         capability.clone(),
//!         ActorId::new("editor"),
//!         AuthorityLevel::Propose,
//!     ))
//!     .build();
//!
//! let sealed = SealedWorkspace::seal(descriptor, &store, &capability)?;
//! assert_eq!(sealed.receipts().len(), sealed.descriptor().records().len());
//! assert!(store.read(sealed.descriptor_identity(), &capability).is_ok());
//! # Ok::<(), Box<dyn core::error::Error>>(())
//! ```

use core::fmt;

use crate::artifact_path::ArtifactHandle;
use crate::components::{Component, WorkspaceDescriptor, WorkspaceDescriptorBuilder};
use crate::publication::{CapabilityToken, PublicationReceipt, PublishRefusal, ReferenceStore};
use crate::snapshot::{NODE_CLASS, Snapshot};

/// Why a workspace was not sealed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealError {
    /// The store refused to publish one of the descriptor's records.
    ///
    /// The identity is the one the *descriptor* derived, which is the name the refused
    /// record would have been fetched under.
    Refused {
        /// The record's identity, as the descriptor named it.
        identity: ArtifactHandle,
        /// Why the store refused.
        refusal: PublishRefusal,
    },
    /// The store named a record differently than the descriptor did.
    ///
    /// The two identity seams disagree, so the bytes are in the store under a name the
    /// descriptor never uses. Refused rather than reconciled: adopting the store's name
    /// would rewrite the artifact's identity, and adopting the descriptor's would claim a
    /// fetch that misses.
    IdentityDisagreement {
        /// The identity the descriptor derived.
        expected: ArtifactHandle,
        /// The identity the store derived from the same bytes.
        published: ArtifactHandle,
    },
}

impl fmt::Display for SealError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused { identity, refusal } => {
                write!(f, "publishing `{identity}` was refused: {refusal}")
            }
            Self::IdentityDisagreement {
                expected,
                published,
            } => write!(
                f,
                "the store named a record `{published}` where the descriptor named it \
                 `{expected}`; the two identity seams disagree and the artifact would not \
                 be fetchable under the name it carries"
            ),
        }
    }
}

impl core::error::Error for SealError {}

/// A workspace frozen and published: what it is named, and the proof it was written.
///
/// A value with no borrow of the store it came from, so it can be handed across a protocol
/// boundary, held past the store's lifetime, or compared with one sealed elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedWorkspace {
    descriptor: WorkspaceDescriptor,
    receipts: Vec<PublicationReceipt>,
}

impl SealedWorkspace {
    /// Publish every record `descriptor` is made of, children before parents.
    ///
    /// One receipt per record in [`WorkspaceDescriptor::records`] order — including the
    /// records the store already held, because a receipt records that *this* publication
    /// happened, and suppressing one for content that converged would make the reported
    /// cost an existence oracle (`publication.rs`, [`PublicationCost`]).
    ///
    /// [`PublicationCost`]: crate::publication::PublicationCost
    ///
    /// # Errors
    ///
    /// [`SealError::Refused`] when the store refuses a record — an insufficient
    /// capability, an identity collision with byte-different content already held, or an
    /// aborted publication — and [`SealError::IdentityDisagreement`] when the store's
    /// identity seam disagrees with the descriptor's.
    pub fn seal(
        descriptor: WorkspaceDescriptor,
        store: &ReferenceStore,
        capability: &CapabilityToken,
    ) -> Result<Self, SealError> {
        let mut receipts = Vec::new();
        for (identity, record) in descriptor.records() {
            let receipt = store
                .publish(NODE_CLASS, record.to_vec(), capability)
                .map_err(|refusal| SealError::Refused {
                    identity: identity.clone(),
                    refusal,
                })?;
            if receipt.handle() != identity {
                return Err(SealError::IdentityDisagreement {
                    expected: identity.clone(),
                    published: receipt.handle().clone(),
                });
            }
            receipts.push(receipt);
        }
        Ok(Self {
            descriptor,
            receipts,
        })
    }

    /// The descriptor that was sealed.
    #[must_use]
    pub const fn descriptor(&self) -> &WorkspaceDescriptor {
        &self.descriptor
    }

    /// The workspace's own identity: the descriptor's.
    #[must_use]
    pub const fn descriptor_identity(&self) -> &ArtifactHandle {
        self.descriptor.identity()
    }

    /// The file tree's identity: the Merkle root's.
    #[must_use]
    pub const fn snapshot_identity(&self) -> &ArtifactHandle {
        self.descriptor.source().identity()
    }

    /// The sealed file tree.
    #[must_use]
    pub const fn snapshot(&self) -> &Snapshot {
        self.descriptor.source()
    }

    /// One receipt per published record, children before parents.
    #[must_use]
    pub fn receipts(&self) -> &[PublicationReceipt] {
        &self.receipts
    }

    /// The receipt of the descriptor record: the root, published last.
    ///
    /// The receipt a consumer ties its "this workspace is sealed" record to
    /// ([`Published`](crate::publication::Published), INV-017). [`None`] is unreachable for
    /// a value [`Self::seal`] returned: every descriptor's records end with its own.
    ///
    /// It is the root's receipt because RFC 0026 makes atomicity per artifact: "a composite
    /// MAY leave earlier records published, never its root". The root's index commit is
    /// the one that makes the workspace fetchable under its own name.
    #[must_use]
    pub fn root_receipt(&self) -> Option<&PublicationReceipt> {
        self.receipts
            .iter()
            .rev()
            .find(|receipt| receipt.handle() == self.descriptor.identity())
    }

    /// The descriptor, taken by value.
    #[must_use]
    pub fn into_descriptor(self) -> WorkspaceDescriptor {
        self.descriptor
    }
}

/// A builder carrying `descriptor`'s components, for binding a different source tree beside
/// them.
///
/// The step between forking and sealing. A fork advances the *file tree*; the lockfile,
/// toolchain pin, and configuration a workspace was imported with do not change when an
/// editor buffer does, so re-deriving them from disk would be both wrong (the disk may
/// have moved on) and impossible (a sealed workspace has no disk). Rebinding takes the
/// components as the bytes they already are.
///
/// It is a plain function rather than a method on [`WorkspaceDescriptor`] because it makes
/// no decision the descriptor owns: it reads three public accessors and calls three public
/// builder methods.
#[must_use]
pub fn rebind_components(descriptor: &WorkspaceDescriptor) -> WorkspaceDescriptorBuilder {
    let mut builder = WorkspaceDescriptorBuilder::default();
    if let Some(component) = descriptor.dependencies() {
        builder = builder.dependencies(Component::content(component).to_vec());
    }
    if let Some(component) = descriptor.toolchain() {
        builder = builder.toolchain(Component::content(component).to_vec());
    }
    if let Some(component) = descriptor.configuration() {
        builder = builder.configuration(Component::content(component).to_vec());
    }
    builder
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_path::ArtifactClass;
    use crate::publication::{
        ActorId, AuditLog, AuthorityLevel, CapabilityDescriptor, ContentIdentifier,
        IdentityUnavailable,
    };
    use crate::snapshot::{WorkspaceContent, WorkspacePath};

    #[derive(Clone)]
    struct HexIdentity;

    impl ContentIdentifier for HexIdentity {
        fn identify(
            &self,
            class: ArtifactClass,
            content: &[u8],
        ) -> Result<ArtifactHandle, IdentityUnavailable> {
            let mut token = String::with_capacity(content.len() * 2);
            for byte in content {
                token.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
                token.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
            }
            ArtifactHandle::new(class, &token).map_err(|_| IdentityUnavailable)
        }
    }

    fn path(text: &str) -> WorkspacePath {
        WorkspacePath::new(text).expect("a test path is valid")
    }

    fn descriptor() -> WorkspaceDescriptor {
        let mut content = WorkspaceContent::new();
        content
            .insert(path("src/lib.rs"), b"fn main() {}".to_vec())
            .expect("insert");
        WorkspaceDescriptor::builder()
            .dependencies(b"[[package]]".to_vec())
            .toolchain(b"[toolchain]".to_vec())
            .build(
                Snapshot::build(&content, &HexIdentity).expect("build"),
                &HexIdentity,
            )
            .expect("descriptor")
    }

    fn store_and_capability() -> (ReferenceStore, CapabilityToken) {
        let capability = CapabilityToken::mint("publisher").expect("token");
        let store = ReferenceStore::builder(HexIdentity, AuditLog::new())
            .capability(CapabilityDescriptor::new(
                capability.clone(),
                ActorId::new("editor"),
                AuthorityLevel::Propose,
            ))
            .build();
        (store, capability)
    }

    #[test]
    fn sealing_publishes_every_record_in_order() {
        let descriptor = descriptor();
        let expected: Vec<ArtifactHandle> = descriptor
            .records()
            .into_iter()
            .map(|(identity, _)| identity.clone())
            .collect();
        let (store, capability) = store_and_capability();

        let sealed = SealedWorkspace::seal(descriptor, &store, &capability).expect("seal");
        let published: Vec<ArtifactHandle> = sealed
            .receipts()
            .iter()
            .map(|receipt| receipt.handle().clone())
            .collect();
        assert_eq!(published, expected);
        assert_eq!(sealed.descriptor_identity(), expected.last().expect("root"));
    }

    /// bn-283p6: the root receipt is the descriptor record's, the last one published, so a
    /// consumer that ties "sealed" to it ties it to the commit that makes the workspace
    /// fetchable under its own name.
    #[test]
    fn the_root_receipt_is_the_descriptor_records() {
        let (store, capability) = store_and_capability();
        let sealed = SealedWorkspace::seal(descriptor(), &store, &capability).expect("seal");
        let root = sealed
            .root_receipt()
            .expect("every seal publishes its descriptor");
        assert_eq!(root.handle(), sealed.descriptor_identity());
        assert_eq!(Some(root), sealed.receipts().last());
        assert_ne!(
            root.handle(),
            sealed.snapshot_identity(),
            "the root is the descriptor, not the file tree"
        );
    }

    #[test]
    fn a_sealed_workspace_is_readable_under_the_names_it_carries() {
        let (store, capability) = store_and_capability();
        let sealed = SealedWorkspace::seal(descriptor(), &store, &capability).expect("seal");

        assert!(
            store
                .read(sealed.descriptor_identity(), &capability)
                .is_ok()
        );
        assert!(store.read(sealed.snapshot_identity(), &capability).is_ok());
    }

    #[test]
    fn rebinding_carries_the_components_and_not_the_tree() {
        let descriptor = descriptor();
        let mut other = WorkspaceContent::new();
        other
            .insert(path("src/other.rs"), b"fn other() {}".to_vec())
            .expect("insert");
        let other = Snapshot::build(&other, &HexIdentity).expect("build");

        let rebound = rebind_components(&descriptor)
            .build(other.clone(), &HexIdentity)
            .expect("rebind");

        assert_eq!(rebound.source().identity(), other.identity());
        assert_eq!(
            rebound.dependencies().map(Component::content),
            descriptor.dependencies().map(Component::content),
        );
        assert_eq!(
            rebound.toolchain().map(Component::content),
            descriptor.toolchain().map(Component::content),
        );
        assert!(rebound.configuration().is_none());
        assert_ne!(rebound.identity(), descriptor.identity());
    }

    #[test]
    fn a_capability_that_cannot_publish_refuses_the_seal() {
        let capability = CapabilityToken::mint("reader").expect("token");
        let store = ReferenceStore::builder(HexIdentity, AuditLog::new())
            .capability(CapabilityDescriptor::new(
                capability.clone(),
                ActorId::new("reader"),
                AuthorityLevel::Read,
            ))
            .build();

        assert!(matches!(
            SealedWorkspace::seal(descriptor(), &store, &capability),
            Err(SealError::Refused {
                refusal: PublishRefusal::CapabilityDenied(_),
                ..
            }),
        ));
    }
}
