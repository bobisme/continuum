//! Who produced a node or an edge, and which producers are services (RFC 0038
//! "Authority", docs/44 "Credit and provenance").
//!
//! # The scheme is the load-bearing half
//!
//! > Actor capabilities control node creation; status promotion is service-restricted.
//! > […] Only trusted services promote into `Validated` or `Proved`. […] Agent votes or
//! > confidence never change status.
//! >
//! > — RFC 0038, "Authority"
//!
//! > Every node records actor/tool/model version, prompts/tool inputs as policy permits,
//! > source retrievals, and derivation. This supports scientific credit, debugging,
//! > benchmark analysis, and reproduction **without granting authority based on identity**.
//! >
//! > — docs/44, "Credit and provenance"
//!
//! Those two sentences pull in opposite directions unless the *scheme* and the *name* are
//! separated. Authority is never a function of who an actor is — `agent:alice` gets nothing
//! `agent:bob` does not — but it is a function of what class of thing the actor is, because
//! every promoting authority in docs/44's table is a **service** ("execution service",
//! "verification service", "independent checker", "Lean proof service") and a promotion into
//! `sampled`/`bounded`/`validated`/`proved` must name a `service_identity`
//! (`evidence-graph-node.schema.json`). [`ActorScheme`] is that class, and
//! [`ServiceIdentity`] is the type an authority decision may take — it exists so that
//! "this argument is a service" is a thing the compiler checks rather than a string the
//! caller promises.
//!
//! The four schemes are the wire's, not this module's invention: `ActorId` in the daemon's
//! protocol scalars declares `^(agent|human|service|ci):[A-Za-z0-9._:-]+$`, and an evidence
//! record whose actor did not round-trip through that alias could not be published.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | four schemes, exactly | daemon `ActorId::SCHEMES` | `the_scheme_set_is_the_wire_alias` |
//! | `scheme:name`, non-empty name, `[A-Za-z0-9._:-]` tail | `ActorId::PATTERN` | `well_formed_actors_round_trip`, `malformed_actors_are_refused` |
//! | promotion is service-restricted | RFC 0038 "Authority" | `only_a_service_scheme_actor_becomes_a_service_identity` |
//! | authority is not a function of identity | docs/44 "Credit and provenance" | `two_agents_differ_only_by_name` |

use core::fmt;

use continuum_value::value::{Name, Value};

/// The class of thing an actor is.
///
/// Declared in the order the daemon's `ActorId::SCHEMES` lists them, which is the IDL's
/// declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActorScheme {
    /// An autonomous agent. May propose; may never promote (RFC 0038: "Agent votes or
    /// confidence never change status").
    Agent,
    /// A person.
    Human,
    /// A service identity. The only scheme docs/44's status-authority table ever names on
    /// the authority side, and the only one an assurance-bearing promotion can attribute.
    Service,
    /// A continuous-integration identity.
    Ci,
}

impl ActorScheme {
    /// Every scheme, in the wire alias's declaration order.
    pub const ALL: [Self; 4] = [Self::Agent, Self::Human, Self::Service, Self::Ci];

    /// The stable wire token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Human => "human",
            Self::Service => "service",
            Self::Ci => "ci",
        }
    }

    /// Parse a scheme token.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == token)
    }

    /// Whether an actor of this scheme may hold a trusted-service role.
    ///
    /// Only [`Service`](Self::Service). Every row of docs/44's authority table names a
    /// service, and the node schema requires a `service_identity` on every
    /// assurance-bearing promotion, so an actor of any other scheme has nothing to write
    /// there.
    #[must_use]
    pub const fn is_service(self) -> bool {
        matches!(self, Self::Service)
    }
}

impl fmt::Display for ActorScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An actor identity, `scheme:name`.
///
/// The spelling is the daemon's `ActorId` alias, `^(agent|human|service|ci):[A-Za-z0-9._:-]+$`.
/// Ordering is byte order on that spelling, so a `BTreeMap` keyed by an actor iterates
/// deterministically and groups by scheme.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActorId {
    text: String,
    scheme: ActorScheme,
}

impl ActorId {
    /// The pattern this type accepts, in the wire alias's own spelling.
    pub const PATTERN: &'static str = "^(agent|human|service|ci):[A-Za-z0-9._:-]+$";

    /// Parse an actor identity.
    ///
    /// # Errors
    ///
    /// [`ActorError`] when the scheme is not one of the four, when the name half is empty,
    /// or when it carries a character outside `[A-Za-z0-9._:-]`.
    pub fn new(text: &str) -> Result<Self, ActorError> {
        let (scheme, name) = text.split_once(':').ok_or(ActorError::NoScheme)?;
        let scheme = ActorScheme::from_token(scheme).ok_or(ActorError::UnknownScheme)?;
        if name.is_empty() {
            return Err(ActorError::EmptyName);
        }
        let permitted =
            |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-');
        if !name.bytes().all(permitted) {
            return Err(ActorError::NameCharacter);
        }
        Ok(Self {
            text: text.to_owned(),
            scheme,
        })
    }

    /// The canonical spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The scheme half.
    #[must_use]
    pub const fn scheme(&self) -> ActorScheme {
        self.scheme
    }

    /// The name half, after the colon.
    #[must_use]
    pub fn name(&self) -> &str {
        let (_, name) = self
            .text
            .split_once(':')
            .expect("a constructed ActorId contains a colon");
        name
    }

    /// This actor as a canonical value, for a content-identity preimage.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::text(self.text.clone())
    }
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// An actor identity whose scheme is [`ActorScheme::Service`].
///
/// This type is the whole of "status promotion is service-restricted" at the level of
/// arguments: an authority decision that takes a `ServiceIdentity` cannot be handed an
/// `agent:` actor, and the only way to obtain one is [`ServiceIdentity::new`], which refuses
/// the other three schemes. It carries no privilege by itself — which service holds which
/// role is [`crate::authority`]'s question — because docs/44 grants authority to roles, not
/// to names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceIdentity(ActorId);

impl ServiceIdentity {
    /// Narrow an actor identity to a service identity.
    ///
    /// # Errors
    ///
    /// [`NotAService`] when the actor's scheme is not `service`.
    pub fn new(actor: ActorId) -> Result<Self, NotAService> {
        if actor.scheme().is_service() {
            Ok(Self(actor))
        } else {
            Err(NotAService {
                scheme: actor.scheme(),
            })
        }
    }

    /// Parse a `service:`-scheme actor identity directly.
    ///
    /// # Errors
    ///
    /// [`ServiceIdentityError`] when the text is not a well-formed actor identity, or when
    /// it is one whose scheme is not `service`.
    pub fn parse(text: &str) -> Result<Self, ServiceIdentityError> {
        let actor = ActorId::new(text).map_err(ServiceIdentityError::Actor)?;
        Self::new(actor).map_err(ServiceIdentityError::Scheme)
    }

    /// The underlying actor identity.
    #[must_use]
    pub const fn actor(&self) -> &ActorId {
        &self.0
    }

    /// The canonical spelling, including the `service:` scheme.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for ServiceIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why an actor identity was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActorError {
    /// The text carries no `scheme:` prefix.
    NoScheme,
    /// The scheme is not one of the four the wire alias declares.
    UnknownScheme,
    /// The name half is empty. An unnamed actor is not an actor.
    EmptyName,
    /// The name half carries a character outside `[A-Za-z0-9._:-]`.
    NameCharacter,
}

impl fmt::Display for ActorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NoScheme => "an actor identity is `scheme:name` and carries no scheme",
            Self::UnknownScheme => "the scheme is not one of agent, human, service, ci",
            Self::EmptyName => "the name half of an actor identity is empty",
            Self::NameCharacter => "the name half carries a character outside [A-Za-z0-9._:-]",
        };
        f.write_str(message)
    }
}

impl core::error::Error for ActorError {}

/// An actor was offered where a service identity is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NotAService {
    /// The scheme the offered actor carries.
    pub scheme: ActorScheme,
}

impl fmt::Display for NotAService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "status promotion is service-restricted: `{}:` is not `service:`",
            self.scheme
        )
    }
}

impl core::error::Error for NotAService {}

/// Why [`ServiceIdentity::parse`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ServiceIdentityError {
    /// The text is not a well-formed actor identity at all.
    Actor(ActorError),
    /// It is a well-formed actor identity of the wrong scheme.
    Scheme(NotAService),
}

impl fmt::Display for ServiceIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Actor(error) => fmt::Display::fmt(error, f),
            Self::Scheme(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl core::error::Error for ServiceIdentityError {}

/// A field name that is a compile-time constant.
///
/// Every call site passes an ASCII-graphic literal, which is exactly what
/// [`Name::new`] accepts, so the failure arm is unreachable rather than unhandled.
pub(crate) fn field(text: &'static str) -> Name {
    Name::new(text).expect("a compile-time field name is printable ASCII")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scheme_set_is_the_wire_alias() {
        // The daemon's `ActorId::SCHEMES`, in its declaration order.
        let tokens: Vec<&str> = ActorScheme::ALL.iter().map(|s| s.as_str()).collect();
        assert_eq!(tokens, ["agent", "human", "service", "ci"]);
        assert_eq!(ActorScheme::from_token("robot"), None);
        assert_eq!(ActorScheme::from_token("Agent"), None);
        assert_eq!(
            ActorId::PATTERN,
            "^(agent|human|service|ci):[A-Za-z0-9._:-]+$"
        );
    }

    #[test]
    fn well_formed_actors_round_trip() {
        for text in [
            "agent:invariant-synthesizer",
            "human:ada",
            "service:continuumd-verifier",
            "ci:nightly.corpus",
            "service:a:b",
        ] {
            let actor = ActorId::new(text).expect("well formed");
            assert_eq!(actor.as_str(), text);
            assert_eq!(actor.to_string(), text);
            let (scheme, name) = text.split_once(':').expect("a scheme");
            assert_eq!(actor.scheme().as_str(), scheme);
            assert_eq!(actor.name(), name);
        }
    }

    #[test]
    fn malformed_actors_are_refused() {
        assert_eq!(ActorId::new("ada"), Err(ActorError::NoScheme));
        assert_eq!(ActorId::new("robot:ada"), Err(ActorError::UnknownScheme));
        assert_eq!(ActorId::new("agent:"), Err(ActorError::EmptyName));
        assert_eq!(ActorId::new("agent:a b"), Err(ActorError::NameCharacter));
        assert_eq!(ActorId::new("agent:a/b"), Err(ActorError::NameCharacter));
        assert_eq!(ActorId::new(""), Err(ActorError::NoScheme));
        // The message names the rule, not the offending actor.
        assert!(ActorError::UnknownScheme.to_string().contains("agent"));
    }

    #[test]
    fn only_a_service_scheme_actor_becomes_a_service_identity() {
        // RFC 0038: "status promotion is service-restricted".
        let service = ServiceIdentity::parse("service:lean-proof").expect("a service");
        assert_eq!(service.as_str(), "service:lean-proof");
        assert_eq!(service.actor().scheme(), ActorScheme::Service);

        for text in ["agent:swarm-1", "human:ada", "ci:nightly"] {
            let actor = ActorId::new(text).expect("well formed");
            let scheme = actor.scheme();
            assert_eq!(ServiceIdentity::new(actor), Err(NotAService { scheme }));
        }
        assert!(matches!(
            ServiceIdentity::parse("agent:swarm-1"),
            Err(ServiceIdentityError::Scheme(_))
        ));
        assert!(matches!(
            ServiceIdentity::parse("not-an-actor"),
            Err(ServiceIdentityError::Actor(_))
        ));
        assert_eq!(
            NotAService {
                scheme: ActorScheme::Agent
            }
            .to_string(),
            "status promotion is service-restricted: `agent:` is not `service:`"
        );
    }

    #[test]
    fn exactly_one_scheme_is_a_service() {
        let services: Vec<ActorScheme> = ActorScheme::ALL
            .into_iter()
            .filter(|scheme| scheme.is_service())
            .collect();
        assert_eq!(services, [ActorScheme::Service]);
    }

    #[test]
    fn two_agents_differ_only_by_name() {
        // docs/44: provenance supports credit "without granting authority based on
        // identity". Nothing about an actor beyond its scheme is consulted anywhere in
        // this module, and the two agents below are interchangeable in every predicate
        // it exposes.
        let alice = ActorId::new("agent:alice").expect("well formed");
        let bob = ActorId::new("agent:bob").expect("well formed");
        assert_ne!(alice, bob);
        assert_eq!(alice.scheme(), bob.scheme());
        assert_eq!(alice.scheme().is_service(), bob.scheme().is_service());
        assert_eq!(
            ServiceIdentity::new(alice).is_err(),
            ServiceIdentity::new(bob).is_err()
        );
    }

    #[test]
    fn actor_order_is_byte_order_on_the_spelling() {
        let mut actors = [
            ActorId::new("service:z").expect("well formed"),
            ActorId::new("agent:b").expect("well formed"),
            ActorId::new("agent:a").expect("well formed"),
            ActorId::new("human:a").expect("well formed"),
        ];
        actors.sort();
        let spelled: Vec<&str> = actors.iter().map(ActorId::as_str).collect();
        assert_eq!(spelled, ["agent:a", "agent:b", "human:a", "service:z"]);
    }

    #[test]
    fn an_actor_renders_as_its_spelling() {
        let actor = ActorId::new("agent:alice").expect("well formed");
        assert_eq!(actor.to_value(), Value::text("agent:alice"));
    }
}
