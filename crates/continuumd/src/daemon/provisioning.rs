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
//!
//! # The instance scope
//!
//! > A member MUST be refused where the capability is provisioned, never registered and
//! > never served as an empty scope, when its prefix names no class; when its class is `ws`
//! > or `in`, whose instances belong in `snapshots` and `intents` (one scope, one place);
//! > when its class is `cap` […]; and when `artifact_classes` is non-empty and does not
//! > hold its class, because that instance could never be admitted.
//! >
//! > — the IDL, `rule capability.instance_scope`
//!
//! [`scoped_instances`] applies that rule. Each refusal is an [`InstanceRefusal`], so a
//! wrong-class member is a typed answer at the provisioning boundary, on the same footing
//! as a mis-spelled class. It is never a scope that silently admits nothing.

use core::fmt;

use continuum_workspace::artifact_path::ArtifactClass;

use crate::protocol::handshake::CapabilityDescriptor;
use crate::protocol::scalar::{ActorId, ArtifactHandle, CapabilityHandle};

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
        /// The capability that was not registered. Typed, for the deployment's own code; the
        /// [`Display`](fmt::Display) rendering never carries it (RFC 0027 S5).
        capability: CapabilityHandle,
        /// The actor the capability was for, which is what the rendering names instead.
        actor: ActorId,
        /// Why the first offending member names no class.
        refusal: ArtifactClassRefusal,
    },
    /// `instances` names a handle that cannot be an instance scope
    /// (`rule capability.instance_scope`).
    Instance {
        /// The capability that was not registered. Typed, for the deployment's own code; the
        /// [`Display`](fmt::Display) rendering never carries it (RFC 0027 S5).
        capability: CapabilityHandle,
        /// The actor the capability was for, which is what the rendering names instead.
        actor: ActorId,
        /// Why the first offending member cannot be scoped.
        refusal: InstanceRefusal,
    },
}

/// Why one `instances` member cannot be an instance scope
/// (`rule capability.instance_scope`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceRefusal {
    /// The handle's prefix names no plan §4.4 class.
    UnknownClass {
        /// The handle as provisioned.
        given: ArtifactHandle,
    },
    /// A `ws_` or `in_` handle. Those classes have their own instance lists, `snapshots`
    /// and `intents`, and one scope has one place.
    DedicatedList {
        /// The handle as provisioned.
        given: ArtifactHandle,
        /// The descriptor field the handle belongs in.
        field: &'static str,
    },
    /// A `cap_` handle. A capability is the authority a request presents, not an artifact
    /// the request names, so it has no instance scope. The handle is a secret (RFC 0027
    /// S5), so the refusal does not carry it.
    Capability,
    /// A handle of a class that a non-empty `artifact_classes` does not hold. T2 would deny
    /// every request that named it, so the member could only ever be an empty scope.
    OutsideClassScope {
        /// The handle as provisioned.
        given: ArtifactHandle,
        /// The class the handle names.
        class: ArtifactClass,
    },
}

impl fmt::Display for InstanceRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownClass { given } => write!(
                f,
                "instance `{}` carries no plan §4.4 class prefix",
                given.as_str()
            ),
            Self::DedicatedList { given, field } => write!(
                f,
                "instance `{}` belongs in `{field}`, not in `instances`",
                given.as_str()
            ),
            Self::Capability => f.write_str(
                "a `cap_` handle is an authority, not an artifact, and has no instance scope",
            ),
            Self::OutsideClassScope { given, class } => write!(
                f,
                "instance `{}` is of class `{}`, which `artifact_classes` does not hold",
                given.as_str(),
                class.token()
            ),
        }
    }
}

impl std::error::Error for InstanceRefusal {}

impl fmt::Display for ProvisioningRefusal {
    /// The refusal as text for a log line or a panic message.
    ///
    /// A `cap_` token is a bearer secret: RFC 0026 and RFC 0027 forbid it in error text, and
    /// [`Builder::build`](super::Builder::build) puts this text in a panic message. So the
    /// rendering names the capability by its actor and a fixed redaction marker, never by
    /// its token (cr-3hcpn4). The token stays in the typed `capability` field, where the
    /// deployment that provisioned it can compare it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (Self::ArtifactClass { actor, .. } | Self::Instance { actor, .. }) = self;
        write!(
            f,
            "the capability for `{}` (token {REDACTED}) was not provisioned: ",
            actor.as_str()
        )?;
        match self {
            Self::ArtifactClass { refusal, .. } => write!(f, "{refusal}"),
            Self::Instance { refusal, .. } => write!(f, "{refusal}"),
        }
    }
}

/// What a rendering prints where a capability token would go.
pub const REDACTED: &str = "<redacted>";

impl std::error::Error for ProvisioningRefusal {}

/// The spelling a refusal may keep: `spelling` itself, or a fixed marker in place of a
/// `cap_` token pasted where a class belongs.
///
/// A capability token is a bearer secret (RFC 0027 S5). A refusal is a value every formatter
/// can reach — `Display`, `{:?}`, `{:#?}`, a panic message — so the token is dropped here, at
/// construction, and no field of any refusal ever holds it (cr-3hcpn4). The prefix alone,
/// `cap_`, is kept as it is: it is the class prefix spelling and names no token.
fn retained(spelling: &str) -> String {
    let prefix = ArtifactClass::Capability.prefix();
    if spelling.starts_with(prefix) && spelling.len() > prefix.len() {
        format!("{prefix}{REDACTED}")
    } else {
        spelling.to_owned()
    }
}

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
            given: retained(spelling),
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
                actor: descriptor.actor.clone(),
                refusal,
            })
        })
        .collect()
}

/// The plan §4.4 class a handle spelling belongs to: the class whose prefix it carries.
///
/// No prefix is a prefix of another (`in_` and `inb_` differ at their third byte), so at
/// most one class matches. Pure in the spelling and consulting no store, which is why
/// admission may call it (RFC 0027 X3).
#[must_use]
pub fn class_of(handle: &str) -> Option<ArtifactClass> {
    ArtifactClass::ALL
        .into_iter()
        .find(|class| handle.starts_with(class.prefix()))
}

/// The instance scope `descriptor` declares, as (class, handle) pairs, or the refusal that
/// keeps it from being provisioned (`rule capability.instance_scope`).
///
/// An absent or empty `instances` list is returned as an empty list: every class then keeps
/// its class scope, which is the 3.6 reading.
///
/// # Errors
///
/// [`ProvisioningRefusal::Instance`] naming the first member that is of no class, of a class
/// with its own instance list (`ws`, `in`), a capability, or of a class a non-empty
/// `artifact_classes` does not hold.
pub fn scoped_instances(
    descriptor: &CapabilityDescriptor,
) -> Result<Vec<(ArtifactClass, ArtifactHandle)>, ProvisioningRefusal> {
    let classes = scoped_classes(descriptor)?;
    let refuse = |refusal| ProvisioningRefusal::Instance {
        capability: descriptor.capability.clone(),
        actor: descriptor.actor.clone(),
        refusal,
    };
    let Some(listed) = descriptor.instances.value() else {
        return Ok(Vec::new());
    };
    listed
        .iter()
        .map(|given| {
            let class = class_of(given.as_str()).ok_or_else(|| {
                refuse(InstanceRefusal::UnknownClass {
                    given: given.clone(),
                })
            })?;
            match class {
                ArtifactClass::WorkspaceSnapshot => Err(refuse(InstanceRefusal::DedicatedList {
                    given: given.clone(),
                    field: "snapshots",
                })),
                ArtifactClass::IntentContract => Err(refuse(InstanceRefusal::DedicatedList {
                    given: given.clone(),
                    field: "intents",
                })),
                ArtifactClass::Capability => Err(refuse(InstanceRefusal::Capability)),
                _ if !classes.is_empty() && !classes.contains(&class) => {
                    Err(refuse(InstanceRefusal::OutsideClassScope {
                        given: given.clone(),
                        class,
                    }))
                }
                _ => Ok((class, given.clone())),
            }
        })
        .collect()
}
