//! Provisioning refusals: the typed answer to a mis-provisioned capability.
//!
//! Capability administration is out of band at major 3 (RFC 0026, "Capability
//! administration"), so a deployment provisions each `cap_*` through
//! [`Builder::capability`](super::Builder::capability) or
//! [`DaemonState::register_capability`](super::state::DaemonState::register_capability).
//! This module decides what those two surfaces refuse.
//!
//! # The artifact-class spelling
//!
//! > A class token is the plan §4.4 prefix without its trailing `_`. […] The prefix
//! > spelling (`ws_`) names no class and is not an alias. […] A capability whose
//! > `artifact_classes` names a string that is not a token MUST be refused where it is
//! > provisioned, with a refusal that names the string and, for a prefix spelling, the
//! > token it meant. A daemon MUST NOT register such a capability, and MUST NOT serve it
//! > as an empty or narrowed scope.
//! >
//! > — the IDL, `rule artifact_class.spelling`
//!
//! Before that rule, `CapabilityDescriptor.artifact_classes` was a bare `list<String>` and
//! its spelling was fixed only by doc comments that disagreed. A capability provisioned
//! with plan §4.4's literal `ws_` was registered, admission compared `ws_` against the
//! class token `ws` and matched nothing, and every call it scoped was refused with the same
//! `CapabilityDenied` as any other denial. The deployment could not see its mistake. The
//! wire field stays `String` inside major 3, because retyping it is
//! `rule versioning.breaking_change` (RFC 0026 F22), so the type that carries the class is
//! here, at the provisioning boundary: [`artifact_class`] turns a spelling into an
//! [`ArtifactClass`] or into an [`ArtifactClassRefusal`], and nothing in between.

use core::fmt;

use continuum_workspace::artifact_path::ArtifactClass;

use crate::protocol::handshake::CapabilityDescriptor;
use crate::protocol::scalar::CapabilityHandle;

/// Why one `artifact_classes` member names no artifact class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactClassRefusal {
    /// The plan §4.4 prefix spelling, trailing `_` included, of a real class. `meant` is
    /// the class it spells, whose [`token`](ArtifactClass::token) is the canonical form.
    PrefixSpelling {
        /// The spelling as provisioned.
        given: String,
        /// The class the prefix spelling names.
        meant: ArtifactClass,
    },
    /// A spelling that is neither a class token nor a class prefix.
    Unknown {
        /// The spelling as provisioned.
        given: String,
    },
}

impl fmt::Display for ArtifactClassRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrefixSpelling { given, meant } => write!(
                f,
                "`{given}` is the plan §4.4 prefix spelling; the class token is `{}`",
                meant.token()
            ),
            Self::Unknown { given } => write!(f, "`{given}` names no plan §4.4 artifact class"),
        }
    }
}

impl std::error::Error for ArtifactClassRefusal {}

/// Why a deployment's capability was refused at provisioning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisioningRefusal {
    /// `artifact_classes` names a string outside `rule artifact_class.spelling`'s tokens.
    ArtifactClass {
        /// The capability that was not registered.
        capability: CapabilityHandle,
        /// Why the first offending member names no class.
        refusal: ArtifactClassRefusal,
    },
}

impl fmt::Display for ProvisioningRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArtifactClass {
                capability,
                refusal,
            } => write!(
                f,
                "capability `{}` was not provisioned: {refusal}",
                capability.as_str()
            ),
        }
    }
}

impl std::error::Error for ProvisioningRefusal {}

/// The artifact class `spelling` names, under `rule artifact_class.spelling`.
///
/// # Errors
///
/// [`ArtifactClassRefusal::PrefixSpelling`] for a class prefix (`ws_`), which is not an
/// alias of the token, and [`ArtifactClassRefusal::Unknown`] for anything else outside the
/// nineteen tokens.
pub fn artifact_class(spelling: &str) -> Result<ArtifactClass, ArtifactClassRefusal> {
    if let Some(class) = ArtifactClass::from_token(spelling) {
        return Ok(class);
    }
    match ArtifactClass::ALL
        .into_iter()
        .find(|class| class.prefix() == spelling)
    {
        Some(meant) => Err(ArtifactClassRefusal::PrefixSpelling {
            given: spelling.to_owned(),
            meant,
        }),
        None => Err(ArtifactClassRefusal::Unknown {
            given: spelling.to_owned(),
        }),
    }
}

/// The classes `descriptor` scopes to, or the refusal that keeps it from being provisioned.
///
/// An empty list is returned as an empty list: "empty means all" is the IDL's own reading,
/// and it is a different fact from a list whose members were all refused, which never
/// reaches a caller as a list at all.
///
/// # Errors
///
/// [`ProvisioningRefusal::ArtifactClass`] naming the first member that is not a class
/// token.
pub fn scoped_classes(
    descriptor: &CapabilityDescriptor,
) -> Result<Vec<ArtifactClass>, ProvisioningRefusal> {
    descriptor
        .artifact_classes
        .iter()
        .map(|spelling| {
            artifact_class(spelling).map_err(|refusal| ProvisioningRefusal::ArtifactClass {
                capability: descriptor.capability.clone(),
                refusal,
            })
        })
        .collect()
}
