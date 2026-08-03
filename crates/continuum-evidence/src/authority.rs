//! Trusted status transitions: who may create which nodes, who may promote a claim, and
//! why an untrusted producer has no path to a trusted status (RFC 0038 "Authority",
//! docs/44 "Status authority", INV-004, PR-7 / IMPL-03).
//!
//! # The two halves RFC 0038's Authority section names
//!
//! > Actor capabilities control node creation; status promotion is service-restricted.
//! > Confidence is metadata. […] Only trusted services promote into `Validated` or `Proved`;
//! > `Sampled` and `Bounded` promotions name the producing engine's service identity; every
//! > `Inconclusive` carries a typed INV-008 reason. Agent votes or confidence never change
//! > status.
//! >
//! > — RFC 0038, "Authority"
//!
//! [`CreationCapability`] is the first half and [`TrustedService`] the second. Nothing here
//! models edge-creation authority, because RFC 0038's Authority section does not cover it
//! and bn-3sypm's F14 says so in as many words; inventing it would answer that bone's
//! question.
//!
//! # The promotion witness, and why it is a type
//!
//! The daemon already models INV-004 this way, and the reasoning transfers exactly:
//!
//! > **The promotion write demands a witness only this module can build.**
//! > `DaemonState::promote_evidence` takes a `Promotion`, whose fields are private to this
//! > module and which has no public constructor. `daemon::observe` — the producer's family —
//! > cannot name a value of that type, so the producer-side code physically cannot reach the
//! > status write. […] the rule is what compiles.
//! >
//! > — `crates/continuumd/src/daemon/evidence.rs`
//!
//! [`Promotion`] here has private fields and no public constructor either. The only way to
//! obtain one is [`TrustedService::promote_to`], [`TrustedService::validate`] or
//! [`TrustedService::inconclusive`], each of which needs a `&TrustedService`, which needs a
//! [`ServiceIdentity`], which refuses every actor scheme but `service:`. So the chain from
//! an `agent:` actor to a promoted status has no first step, and
//! [`crate::graph::EvidenceGraph::promote`] — the only function that writes a status — takes
//! a `&Promotion` and nothing else.
//!
//! This crate does **not** import the daemon. plan §20's dependency islands put
//! `continuumd` at this crate's own tier, and an evidence crate that depended on the daemon
//! would invert the graph. The pattern is mirrored; the code is not shared.
//!
//! # PR 7's exit sentence, and the four ways it is held
//!
//! > **Exit:** an untrusted client cannot promote a proposal to validated/proved.
//! >
//! > — `START_HERE_IMPLEMENTATION.md`, PR 7
//!
//! 1. **No status field on the producer's path.** [`crate::node::EvidenceNode::propose`]
//!    takes no status; a node lands at [`ClaimStatus::BOTTOM`].
//! 2. **No promotion witness without a service.** [`Promotion`]'s fields are private and
//!    every constructor requires a [`TrustedService`].
//! 3. **No service identity without the `service:` scheme.** [`ServiceIdentity::new`]
//!    refuses `agent:`, `human:` and `ci:`.
//! 4. **No role reaching `validated`/`proved` except the two docs/44 names.**
//!    [`ServiceRole::may_promote_to`] is the table below, and an execution service that
//!    asks for `validated` is refused by name.
//!
//! A fifth guard is [`crate::graph::EvidenceGraph::promote`]'s own: a service may not
//! promote a claim it produced, which is INV-004's other half and the one the four above do
//! not cover — a deployment that ingests under the identity it verifies under obeys the wire
//! at every step and is still self-certifying.
//!
//! # The authority table
//!
//! docs/44's "Status authority" rows, one cell per row. Two rows are a *reading* rather than
//! a quotation and are marked as such:
//!
//! | Status | Roles | Source |
//! |---|---|---|
//! | `proposed` | none — creation is not a promotion | `(creation) → proposed \| any authorized human/agent` |
//! | `observed` | [`Execution`](ServiceRole::Execution) | `proposed → observed \| execution service` |
//! | `sampled` | `Execution`, [`Verification`](ServiceRole::Verification) | `proposed → sampled \| execution/verification service` |
//! | `bounded` | `Verification` | `proposed → bounded \| verification service` |
//! | `validated` | [`IndependentChecker`](ServiceRole::IndependentChecker) | `proposed/bounded → validated \| independent checker` |
//! | `proved` | [`ProofService`](ServiceRole::ProofService) | `proposed → proved \| Lean proof service` |
//! | `refuted` | `Verification`, `IndependentChecker`, `ProofService` | *reading* of `any → refuted \| valid counterexample/checker`: the three roles that check |
//! | `inconclusive` | `Execution`, `Verification`, `IndependentChecker`, `ProofService` | *reading* of `any → inconclusive \| producing service`: the four roles that run an attempt |
//! | `superseded` | [`PolicyOwner`](ServiceRole::PolicyOwner) | `any → superseded \| policy/owner with explicit edge` |
//!
//! The two readings are marked because they are the only cells docs/44 does not name a role
//! for outright, and a reader auditing this table should know which two to argue with.
//!
//! # Authority is not the lattice
//!
//! [`crate::claim_status::compare_and_set`] answers "would this write lower the claim?" and
//! this module answers "may this writer install that status at all?". Both must hold: a
//! `Lean proof service` promoting `proved` over a `refuted` claim is authorized and is still
//! a regression, and [`crate::graph`]'s `authority_and_the_lattice_are_independent_guards`
//! pins that the two guards are independent.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | only trusted services promote into validated/proved | RFC 0038 | `only_two_roles_reach_validated_and_proved`, and the module's `compile_fail`s |
//! | agent votes never change status | RFC 0038 | `an_agent_has_no_path_to_a_promotion` |
//! | the authority table | docs/44 | `the_authority_table_is_docs_44` |
//! | `sampled`/`bounded`/`validated`/`proved` name a service | node schema | `only_the_four_assurance_statuses_name_a_service` |
//! | every `inconclusive` carries a typed reason | INV-008, plan §11.4 | `an_inconclusive_promotion_carries_a_typed_reason` |
//! | `validated` records its basis | plan §11.4 | `a_validated_promotion_records_its_basis` |
//! | actor capabilities control node creation | RFC 0038, docs/44 | `an_actor_appends_only_allowed_node_types`, `a_producer_cannot_append_under_another_name` |
//! | a producer may not promote its own claim | INV-004 | `crate::graph`'s `a_service_cannot_promote_its_own_production` |
//!
//! # Three things a caller cannot spell
//!
//! A promotion cannot be built by hand — the fields are private:
//!
//! ```compile_fail
//! use continuum_evidence::authority::Promotion;
//! use continuum_evidence::claim_status::ClaimStatus;
//!
//! let _ = Promotion {
//!     status: ClaimStatus::Proved,
//!     service: "service:me".to_owned(),
//!     validation_basis: None,
//!     inconclusive_reason: None,
//! };
//! ```
//!
//! A promoted status write cannot be minted outside this crate — the constructor is
//! `pub(crate)`, and a doc test compiles as an external crate:
//!
//! ```compile_fail
//! use continuum_evidence::claim_status::ClaimStatus;
//! use continuum_evidence::node::StatusWrite;
//!
//! let _ = StatusWrite::promoted(ClaimStatus::Validated, None, None, None);
//! ```
//!
//! And an agent cannot become a trusted service — the argument is a [`ServiceIdentity`]:
//!
//! ```compile_fail
//! use continuum_evidence::actor::ActorId;
//! use continuum_evidence::authority::{ServiceRole, TrustedService};
//!
//! let agent = ActorId::new("agent:swarm-1").unwrap();
//! let _ = TrustedService::new(agent, [ServiceRole::IndependentChecker]);
//! ```

use core::fmt;
use std::collections::BTreeSet;

use continuum_value::assurance::{InconclusiveReason, ValidationBasis};

use crate::actor::{ActorId, ServiceIdentity};
use crate::claim_status::ClaimStatus;
use crate::node::{EvidenceNode, NodeKind, StatusWrite};

/// The strongest status reachable by an actor holding no trusted-service role.
///
/// The bottom of the lattice: creation lands there and there is no constructor, anywhere in
/// this crate's public API, that takes an untrusted actor to a higher one. It is a constant
/// rather than a function because there is no computation — the set of promotions available
/// without a [`TrustedService`] is empty.
pub const UNTRUSTED_CEILING: ClaimStatus = ClaimStatus::BOTTOM;

/// The statuses whose promotion the node schema requires to name a service identity.
///
/// > if `status` in `[sampled, bounded, validated, proved]` then `service_identity` is
/// > required
/// >
/// > — `evidence-graph-node.schema.json`
pub const SERVICE_ATTRIBUTED: [ClaimStatus; 4] = [
    ClaimStatus::Sampled,
    ClaimStatus::Bounded,
    ClaimStatus::Validated,
    ClaimStatus::Proved,
];

/// A role docs/44's status-authority table grants a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ServiceRole {
    /// "execution service" — it runs things and reports what happened.
    Execution,
    /// "verification service" — it explores within a stated envelope.
    Verification,
    /// "independent checker" — it re-checks a certificate produced elsewhere.
    IndependentChecker,
    /// "Lean proof service".
    ProofService,
    /// "policy/owner" — it retires claims in favour of successors.
    PolicyOwner,
}

impl ServiceRole {
    /// Every role, in the order docs/44's rows introduce them.
    pub const ALL: [Self; 5] = [
        Self::Execution,
        Self::Verification,
        Self::IndependentChecker,
        Self::ProofService,
        Self::PolicyOwner,
    ];

    /// The stable machine name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Execution => "execution",
            Self::Verification => "verification",
            Self::IndependentChecker => "independent-checker",
            Self::ProofService => "proof-service",
            Self::PolicyOwner => "policy-owner",
        }
    }

    /// Whether this role may install `status`, per the module's authority table.
    ///
    /// `proposed` is false for every role: creation is not a promotion, and docs/44 gives
    /// its row to "any authorized human/agent" rather than to a service.
    #[must_use]
    pub const fn may_promote_to(self, status: ClaimStatus) -> bool {
        match status {
            ClaimStatus::Proposed => false,
            ClaimStatus::Observed => matches!(self, Self::Execution),
            ClaimStatus::Sampled => matches!(self, Self::Execution | Self::Verification),
            ClaimStatus::Bounded => matches!(self, Self::Verification),
            ClaimStatus::Validated => matches!(self, Self::IndependentChecker),
            ClaimStatus::Proved => matches!(self, Self::ProofService),
            ClaimStatus::Refuted => matches!(
                self,
                Self::Verification | Self::IndependentChecker | Self::ProofService
            ),
            ClaimStatus::Inconclusive => !matches!(self, Self::PolicyOwner),
            ClaimStatus::Superseded => matches!(self, Self::PolicyOwner),
        }
    }
}

impl fmt::Display for ServiceRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which node kinds an actor may append (RFC 0038: "Actor capabilities control node
/// creation"; docs/44: "actors may append only allowed node types").
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CreatableKinds {
    /// Every kind. The deployment has made no restriction.
    #[default]
    Any,
    /// Exactly these kinds.
    OneOf(BTreeSet<NodeKind>),
}

impl CreatableKinds {
    /// The set that admits exactly these kinds.
    #[must_use]
    pub fn one_of(kinds: impl IntoIterator<Item = NodeKind>) -> Self {
        Self::OneOf(kinds.into_iter().collect())
    }

    /// Whether a kind is admitted.
    #[must_use]
    pub fn admits(&self, kind: NodeKind) -> bool {
        match self {
            Self::Any => true,
            Self::OneOf(kinds) => kinds.contains(&kind),
        }
    }
}

/// One actor's right to append nodes.
///
/// Deliberately not a right to *promote*: docs/44 separates the two columns and RFC 0038
/// separates the two sentences, so a capability that granted both would be the conflation
/// INV-004 exists to prevent. Promotion needs a [`TrustedService`], which this type cannot
/// produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationCapability {
    actor: ActorId,
    creatable: CreatableKinds,
}

impl CreationCapability {
    /// The capability of one actor over one set of node kinds.
    #[must_use]
    pub const fn new(actor: ActorId, creatable: CreatableKinds) -> Self {
        Self { actor, creatable }
    }

    /// The actor this capability belongs to.
    #[must_use]
    pub const fn actor(&self) -> &ActorId {
        &self.actor
    }

    /// The kinds it may append.
    #[must_use]
    pub const fn creatable(&self) -> &CreatableKinds {
        &self.creatable
    }

    /// Whether this capability admits a node kind.
    #[must_use]
    pub fn may_create(&self, kind: NodeKind) -> bool {
        self.creatable.admits(kind)
    }

    /// Whether this capability authorizes appending exactly this node.
    ///
    /// Two conditions, and the second is the one a capability check usually forgets: the
    /// node's `provenance.actor` must be the capability's own actor. The daemon gets this
    /// from the wire — it writes `provenance.actor` "from the *admitted grant*, not from the
    /// request", so "a producer cannot append under someone else's name" — and a library
    /// whose caller supplies the provenance has to state it.
    ///
    /// # Errors
    ///
    /// - [`AuthorityError::CreationNotAuthorized`] when the kind is outside the capability.
    /// - [`AuthorityError::ActorMismatch`] when the node names a different producer.
    pub fn authorize(&self, node: &EvidenceNode) -> Result<(), AuthorityError> {
        if !self.may_create(node.kind()) {
            return Err(AuthorityError::CreationNotAuthorized { kind: node.kind() });
        }
        if node.provenance().actor() != &self.actor {
            return Err(AuthorityError::ActorMismatch);
        }
        Ok(())
    }
}

/// A service identity together with the roles a deployment has granted it.
///
/// Constructing one **is** the deployment's trust declaration; this crate cannot decide who
/// is trusted and does not pretend to. What it does guarantee is that the declaration is
/// required, that it can only name a `service:` actor, and that the roles bound which
/// statuses the service can reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedService {
    identity: ServiceIdentity,
    roles: BTreeSet<ServiceRole>,
}

impl TrustedService {
    /// Declare a trusted service.
    ///
    /// # Errors
    ///
    /// [`AuthorityError::NoRoles`] when the role set is empty. A service trusted for nothing
    /// is not a trusted service, and admitting one would create a value whose only use is to
    /// be refused later — an affordance for "the check passed because there was nothing to
    /// check".
    pub fn new(
        identity: ServiceIdentity,
        roles: impl IntoIterator<Item = ServiceRole>,
    ) -> Result<Self, AuthorityError> {
        let roles: BTreeSet<ServiceRole> = roles.into_iter().collect();
        if roles.is_empty() {
            return Err(AuthorityError::NoRoles);
        }
        Ok(Self { identity, roles })
    }

    /// The service's identity.
    #[must_use]
    pub const fn identity(&self) -> &ServiceIdentity {
        &self.identity
    }

    /// Its roles, in canonical order.
    pub fn roles(&self) -> impl ExactSizeIterator<Item = &ServiceRole> {
        self.roles.iter()
    }

    /// Whether any of its roles may install `status`.
    #[must_use]
    pub fn may_promote_to(&self, status: ClaimStatus) -> bool {
        self.roles.iter().any(|role| role.may_promote_to(status))
    }

    /// Mint a promotion to a status that carries no payload.
    ///
    /// # Errors
    ///
    /// - [`AuthorityError::PayloadRequired`] for `validated` and `inconclusive`, which have
    ///   their own constructors because plan §11.4 attaches a mandatory payload to each.
    /// - [`AuthorityError::NotAuthorized`] when no role of this service may install the
    ///   status — including `proposed`, which is creation rather than promotion.
    pub fn promote_to(&self, status: ClaimStatus) -> Result<Promotion, AuthorityError> {
        match status {
            ClaimStatus::Validated | ClaimStatus::Inconclusive => {
                Err(AuthorityError::PayloadRequired { status })
            }
            _ => self.mint(status, None, None),
        }
    }

    /// Mint a promotion to `validated`, recording the basis plan §11.4 requires.
    ///
    /// > `Validated` records whether solver evidence is `CHECKED_CERTIFICATE` or
    /// > `TRUSTED_SOLVER`; the two never render identically.
    /// >
    /// > — plan §11.4
    ///
    /// # Errors
    ///
    /// [`AuthorityError::NotAuthorized`] unless this service holds
    /// [`ServiceRole::IndependentChecker`].
    pub fn validate(&self, basis: ValidationBasis) -> Result<Promotion, AuthorityError> {
        self.mint(ClaimStatus::Validated, Some(basis), None)
    }

    /// Mint a promotion to `inconclusive`, carrying the typed INV-008 reason.
    ///
    /// > every `Inconclusive` carries a typed INV-008 reason
    /// >
    /// > — RFC 0038, "Authority"
    ///
    /// # Errors
    ///
    /// [`AuthorityError::NotAuthorized`] for a service whose only role is
    /// [`ServiceRole::PolicyOwner`], which runs no attempt and therefore produces no
    /// inconclusive result.
    pub fn inconclusive(&self, reason: InconclusiveReason) -> Result<Promotion, AuthorityError> {
        self.mint(ClaimStatus::Inconclusive, None, Some(reason))
    }

    fn mint(
        &self,
        status: ClaimStatus,
        validation_basis: Option<ValidationBasis>,
        inconclusive_reason: Option<InconclusiveReason>,
    ) -> Result<Promotion, AuthorityError> {
        if !self.may_promote_to(status) {
            return Err(AuthorityError::NotAuthorized {
                status,
                service: self.identity.as_str().to_owned(),
            });
        }
        Ok(Promotion {
            status,
            service: self.identity.clone(),
            validation_basis,
            inconclusive_reason,
        })
    }
}

/// The right to advance one claim's status.
///
/// Its fields are private to this module and it has no public constructor, so the only code
/// that can produce one is [`TrustedService`] above. Every other module — including
/// [`crate::node`], where a producer's append happens — can *name* the type and can never
/// build a value of it, which is what makes "status promotion is service-restricted"
/// (RFC 0038) a compile-time fact rather than a rule someone has to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Promotion {
    status: ClaimStatus,
    service: ServiceIdentity,
    validation_basis: Option<ValidationBasis>,
    inconclusive_reason: Option<InconclusiveReason>,
}

impl Promotion {
    /// The status this promotion would install.
    #[must_use]
    pub const fn status(&self) -> ClaimStatus {
        self.status
    }

    /// The service performing it.
    #[must_use]
    pub const fn service(&self) -> &ServiceIdentity {
        &self.service
    }

    /// The basis it records, present exactly for `validated`.
    #[must_use]
    pub const fn validation_basis(&self) -> Option<ValidationBasis> {
        self.validation_basis
    }

    /// The typed INV-008 reason it carries, present exactly for `inconclusive`.
    #[must_use]
    pub const fn inconclusive_reason(&self) -> Option<InconclusiveReason> {
        self.inconclusive_reason
    }

    /// The append-only record of this write, at the status the compare-and-set settled on.
    ///
    /// `settled` rather than [`status`](Self::status) because a reassertion settles at the
    /// claim's existing status, and the history must record what the claim *holds* rather
    /// than what a caller asked for.
    ///
    /// The service identity is attached only for the four [`SERVICE_ATTRIBUTED`] statuses:
    /// naming one on `observed` would put a field in the node the schema does not admit
    /// there, and `additionalProperties` is false.
    #[must_use]
    pub fn write(&self, settled: ClaimStatus) -> StatusWrite {
        StatusWrite::promoted(
            settled,
            SERVICE_ATTRIBUTED
                .contains(&settled)
                .then(|| self.service.as_str().to_owned()),
            self.validation_basis,
            self.inconclusive_reason,
        )
    }
}

/// Why an authority decision refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityError {
    /// A trusted service was declared with no roles.
    NoRoles,
    /// No role of this service may install this status.
    NotAuthorized {
        /// The status asked for.
        status: ClaimStatus,
        /// The service that asked.
        service: String,
    },
    /// The status carries a mandatory payload and a payload-free constructor was used.
    PayloadRequired {
        /// The status that requires one.
        status: ClaimStatus,
    },
    /// The actor may not append nodes of this kind.
    CreationNotAuthorized {
        /// The kind that was refused.
        kind: NodeKind,
    },
    /// The node names a producer other than the capability's actor.
    ActorMismatch,
}

impl fmt::Display for AuthorityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRoles => f.write_str("a trusted service holds at least one role"),
            Self::NotAuthorized { status, service } => write!(
                f,
                "{service} holds no role authorized to promote into {status}"
            ),
            Self::PayloadRequired { status } => write!(
                f,
                "a promotion into {status} carries a mandatory payload and has its own constructor"
            ),
            Self::CreationNotAuthorized { kind } => {
                write!(f, "this actor may not append `{kind}` nodes")
            }
            Self::ActorMismatch => {
                f.write_str("the node names a producer other than this capability's actor")
            }
        }
    }
}

impl core::error::Error for AuthorityError {}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::node::tests::{node, provenance};
    use crate::node::{ClaimId, EvidenceNode, IdempotencyKey};
    use crate::provenance::ArtifactRef;

    pub(crate) fn service(name: &str, roles: &[ServiceRole]) -> TrustedService {
        TrustedService::new(
            ServiceIdentity::parse(name).expect("a service"),
            roles.iter().copied(),
        )
        .expect("at least one role")
    }

    #[test]
    fn the_authority_table_is_docs_44() {
        // Every cell, spelled out once here so the table in the module docs is checked
        // rather than described.
        let expected: [(ClaimStatus, &[ServiceRole]); 9] = [
            (ClaimStatus::Proposed, &[]),
            (ClaimStatus::Observed, &[ServiceRole::Execution]),
            (
                ClaimStatus::Sampled,
                &[ServiceRole::Execution, ServiceRole::Verification],
            ),
            (ClaimStatus::Bounded, &[ServiceRole::Verification]),
            (ClaimStatus::Validated, &[ServiceRole::IndependentChecker]),
            (ClaimStatus::Proved, &[ServiceRole::ProofService]),
            (
                ClaimStatus::Refuted,
                &[
                    ServiceRole::Verification,
                    ServiceRole::IndependentChecker,
                    ServiceRole::ProofService,
                ],
            ),
            (
                ClaimStatus::Inconclusive,
                &[
                    ServiceRole::Execution,
                    ServiceRole::Verification,
                    ServiceRole::IndependentChecker,
                    ServiceRole::ProofService,
                ],
            ),
            (ClaimStatus::Superseded, &[ServiceRole::PolicyOwner]),
        ];
        assert_eq!(expected.len(), ClaimStatus::ALL.len());
        for (status, authorized) in expected {
            for role in ServiceRole::ALL {
                assert_eq!(
                    role.may_promote_to(status),
                    authorized.contains(&role),
                    "{role} → {status}"
                );
            }
        }
        // Role tokens are stable.
        let tokens: Vec<&str> = ServiceRole::ALL.iter().map(|r| r.as_str()).collect();
        assert_eq!(
            tokens,
            [
                "execution",
                "verification",
                "independent-checker",
                "proof-service",
                "policy-owner"
            ]
        );
    }

    #[test]
    fn only_two_roles_reach_validated_and_proved() {
        // RFC 0038: "Only trusted services promote into `Validated` or `Proved`" — and only
        // the two docs/44 names, so an execution service is refused by name.
        for role in ServiceRole::ALL {
            let service = service("service:s", &[role]);
            assert_eq!(
                service
                    .validate(ValidationBasis::CheckedCertificate)
                    .is_ok(),
                role == ServiceRole::IndependentChecker,
                "{role} → validated"
            );
            assert_eq!(
                service.promote_to(ClaimStatus::Proved).is_ok(),
                role == ServiceRole::ProofService,
                "{role} → proved"
            );
        }
        let execution = service("service:runner", &[ServiceRole::Execution]);
        assert_eq!(
            execution.promote_to(ClaimStatus::Proved),
            Err(AuthorityError::NotAuthorized {
                status: ClaimStatus::Proved,
                service: "service:runner".to_owned(),
            })
        );
        assert_eq!(
            execution
                .promote_to(ClaimStatus::Proved)
                .unwrap_err()
                .to_string(),
            "service:runner holds no role authorized to promote into proved"
        );
    }

    #[test]
    fn an_agent_has_no_path_to_a_promotion() {
        // RFC 0038: "Agent votes or confidence never change status." The structural reading:
        // there is no value of `ServiceIdentity` for a non-service actor, so there is no
        // `TrustedService`, so there is no `Promotion`. The module's third `compile_fail`
        // holds the type-level half; this holds the value-level one.
        for text in ["agent:swarm-1", "human:ada", "ci:nightly"] {
            let actor = ActorId::new(text).expect("well formed");
            assert!(ServiceIdentity::new(actor).is_err(), "{text}");
        }
        assert_eq!(UNTRUSTED_CEILING, ClaimStatus::Proposed);
        assert_eq!(UNTRUSTED_CEILING, ClaimStatus::BOTTOM);
        // …and creation is not a promotion for anyone, service or not.
        for role in ServiceRole::ALL {
            assert!(!role.may_promote_to(ClaimStatus::Proposed), "{role}");
        }
    }

    #[test]
    fn a_service_with_no_roles_is_refused() {
        assert_eq!(
            TrustedService::new(ServiceIdentity::parse("service:s").expect("a service"), []),
            Err(AuthorityError::NoRoles)
        );
        assert_eq!(
            AuthorityError::NoRoles.to_string(),
            "a trusted service holds at least one role"
        );
    }

    #[test]
    fn only_the_four_assurance_statuses_name_a_service() {
        // `evidence-graph-node.schema.json`: `service_identity` is required for exactly
        // sampled/bounded/validated/proved, and `additionalProperties` is false, so writing
        // it anywhere else is as wrong as omitting it there.
        let all_roles = service("service:everything", &ServiceRole::ALL);
        for status in ClaimStatus::ALL {
            let promotion = match status {
                ClaimStatus::Proposed => continue,
                ClaimStatus::Validated => all_roles
                    .validate(ValidationBasis::TrustedSolver)
                    .expect("authorized"),
                ClaimStatus::Inconclusive => all_roles
                    .inconclusive(InconclusiveReason::ResourceExhausted)
                    .expect("authorized"),
                other => all_roles.promote_to(other).expect("authorized"),
            };
            let write = promotion.write(status);
            assert_eq!(
                write.service_identity().is_some(),
                SERVICE_ATTRIBUTED.contains(&status),
                "{status}"
            );
            assert_eq!(write.status(), status);
        }
        assert_eq!(
            SERVICE_ATTRIBUTED,
            [
                ClaimStatus::Sampled,
                ClaimStatus::Bounded,
                ClaimStatus::Validated,
                ClaimStatus::Proved
            ]
        );
    }

    #[test]
    fn a_write_records_the_settled_status_not_the_asked_one() {
        // A reassertion settles where the claim already is; the history must say what the
        // claim holds.
        let checker = service("service:checker", &[ServiceRole::IndependentChecker]);
        let promotion = checker
            .validate(ValidationBasis::CheckedCertificate)
            .expect("authorized");
        assert_eq!(promotion.status(), ClaimStatus::Validated);
        let settled = promotion.write(ClaimStatus::Proved);
        assert_eq!(settled.status(), ClaimStatus::Proved);
        assert_eq!(settled.service_identity(), Some("service:checker"));
    }

    #[test]
    fn a_validated_promotion_records_its_basis() {
        // plan §11.4: "the two never render identically", so there is no payload-free path
        // to `validated`.
        let checker = service("service:checker", &[ServiceRole::IndependentChecker]);
        assert_eq!(
            checker.promote_to(ClaimStatus::Validated),
            Err(AuthorityError::PayloadRequired {
                status: ClaimStatus::Validated
            })
        );
        for basis in [
            ValidationBasis::CheckedCertificate,
            ValidationBasis::TrustedSolver,
        ] {
            let promotion = checker.validate(basis).expect("authorized");
            assert_eq!(promotion.validation_basis(), Some(basis));
            assert_eq!(promotion.inconclusive_reason(), None);
            assert_eq!(
                promotion.write(ClaimStatus::Validated).validation_basis(),
                Some(basis)
            );
        }
    }

    #[test]
    fn an_inconclusive_promotion_carries_a_typed_reason() {
        // INV-008, plan §11.4: "Budget exhaustion is never a verdict", and an untyped
        // inconclusive is the shape that would make it one.
        let engine = service("service:engine", &[ServiceRole::Verification]);
        assert_eq!(
            engine.promote_to(ClaimStatus::Inconclusive),
            Err(AuthorityError::PayloadRequired {
                status: ClaimStatus::Inconclusive
            })
        );
        let promotion = engine
            .inconclusive(InconclusiveReason::ResourceExhausted)
            .expect("authorized");
        assert_eq!(
            promotion.inconclusive_reason(),
            Some(InconclusiveReason::ResourceExhausted)
        );
        assert_eq!(promotion.validation_basis(), None);
        // A policy owner runs no attempt and therefore produces no inconclusive result.
        let owner = service("service:owner", &[ServiceRole::PolicyOwner]);
        assert!(owner.inconclusive(InconclusiveReason::EngineError).is_err());
    }

    #[test]
    fn an_actor_appends_only_allowed_node_types() {
        // docs/44, "Security": "actors may append only allowed node types".
        let actor = ActorId::new("agent:swarm-1").expect("well formed");
        let capability = CreationCapability::new(
            actor.clone(),
            CreatableKinds::one_of([NodeKind::InvariantCandidate, NodeKind::Candidate]),
        );
        assert_eq!(capability.actor(), &actor);
        for kind in NodeKind::ALL {
            let admitted = matches!(kind, NodeKind::InvariantCandidate | NodeKind::Candidate);
            assert_eq!(capability.may_create(kind), admitted, "{kind}");
            let proposed = EvidenceNode::propose(
                kind,
                ArtifactRef::new("cand_9f").expect("well formed"),
                ClaimId::new("claim-1").expect("non-empty"),
                IdempotencyKey::new("key-1").expect("non-empty"),
                provenance("agent:swarm-1"),
            );
            assert_eq!(capability.authorize(&proposed).is_ok(), admitted, "{kind}");
        }
        // The unrestricted capability admits all twenty, so the restriction above is the
        // thing being tested rather than a property of `authorize`.
        let open = CreationCapability::new(actor, CreatableKinds::Any);
        for kind in NodeKind::ALL {
            assert!(open.may_create(kind), "{kind}");
        }
        assert_eq!(
            AuthorityError::CreationNotAuthorized {
                kind: NodeKind::Patch
            }
            .to_string(),
            "this actor may not append `patch` nodes"
        );
    }

    #[test]
    fn a_producer_cannot_append_under_another_name() {
        // The daemon writes `provenance.actor` from the admitted grant; a library whose
        // caller supplies the provenance has to check the same thing.
        let capability = CreationCapability::new(
            ActorId::new("agent:swarm-1").expect("well formed"),
            CreatableKinds::Any,
        );
        let mine = node(NodeKind::Run, "trace_9f", "claim-1");
        assert_eq!(capability.authorize(&mine), Ok(()));
        let theirs = EvidenceNode::propose(
            NodeKind::Run,
            ArtifactRef::new("trace_9f").expect("well formed"),
            ClaimId::new("claim-1").expect("non-empty"),
            IdempotencyKey::new("key-1").expect("non-empty"),
            provenance("agent:swarm-2"),
        );
        assert_eq!(
            capability.authorize(&theirs),
            Err(AuthorityError::ActorMismatch)
        );
    }

    #[test]
    fn a_creation_capability_grants_no_promotion() {
        // The two columns of docs/44's table stay apart: this type has no method that
        // produces a `Promotion`, and the only ones that do need a `TrustedService`.
        let capability = CreationCapability::new(
            ActorId::new("agent:swarm-1").expect("well formed"),
            CreatableKinds::Any,
        );
        assert!(capability.may_create(NodeKind::Run));
        // The actor behind an unrestricted creation capability still cannot be a service.
        assert!(ServiceIdentity::new(capability.actor().clone()).is_err());
    }
}
