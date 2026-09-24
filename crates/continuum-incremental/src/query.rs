//! The query-definition registry and the canonical query key (RFC 0030, "Query key"
//! and "Query definition registry").
//!
//! Every memoized computation is a registered [`QueryDefinition`]. The registry is
//! [`REGISTRY`]: a closed, enumerable list, because `query.explain_invalidation`
//! reports `function_id` values and an engine that cannot enumerate its definitions
//! cannot satisfy the explain contract.

use core::fmt;

use continuum_value::identity::{Blake3Hasher, ContentHasher, DIGEST_LEN, Digest256};

use crate::output::field;
use crate::vocab::{Auditability, DependencyReason, ReuseClass};

/// The stages of the spike's pipeline, in dependency order. The derived order is
/// the deterministic report order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    /// The source text: the one input, not a query.
    Source,
    /// `parse`: source text to the span-free syntax tree.
    Parse,
    /// `elaborate`: the syntax tree to the normalized model.
    Elaborate,
    /// `build_model`: the normalized model to the programmatic model (lowering).
    BuildModel,
    /// `project_transition_system`: the normalized model without its invariants.
    ProjectTransitionSystem,
    /// `project_invariant`: one invariant's body, without its name.
    ProjectInvariant,
    /// `explore`: breadth-first exploration within the configured bounds.
    Explore,
    /// `domain_safety`: the state-domain safety claim.
    DomainSafety,
    /// `check_invariant`: one invariant over the exploration.
    CheckInvariant,
}

impl Stage {
    /// The registered definition of this stage, or `None` for [`Stage::Source`].
    #[must_use]
    pub fn definition(self) -> Option<&'static QueryDefinition> {
        REGISTRY.iter().find(|definition| definition.stage == self)
    }

    /// The report token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Parse => "parse",
            Self::Elaborate => "elaborate",
            Self::BuildModel => "build_model",
            Self::ProjectTransitionSystem => "project_transition_system",
            Self::ProjectInvariant => "project_invariant",
            Self::Explore => "explore",
            Self::DomainSafety => "domain_safety",
            Self::CheckInvariant => "check_invariant",
        }
    }

    /// Whether the Incremental Parity Audit compares this stage's output
    /// (RFC 0030, "Compared artifacts"). The projections and the lowered model are
    /// internal: the clean recomputation never projects, so they have no clean
    /// counterpart, and what they feed is compared downstream.
    #[must_use]
    pub const fn compared(self) -> Compared {
        match self {
            Self::Parse
            | Self::Elaborate
            | Self::Explore
            | Self::DomainSafety
            | Self::CheckInvariant => Compared::Yes,
            Self::Source
            | Self::BuildModel
            | Self::ProjectTransitionSystem
            | Self::ProjectInvariant => Compared::Internal,
        }
    }
}

/// Whether a stage's output is compared by the audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compared {
    /// Compared against the clean recomputation.
    Yes,
    /// Internal to the engine; compared through what it feeds.
    Internal,
}

/// One query instance: a stage, and the invariant name for the per-invariant stages.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Label {
    /// The stage.
    pub stage: Stage,
    /// The invariant name, for [`Stage::ProjectInvariant`] and
    /// [`Stage::CheckInvariant`]; empty otherwise.
    pub name: String,
}

impl Label {
    /// A stage-level label.
    #[must_use]
    pub fn of(stage: Stage) -> Self {
        Self {
            stage,
            name: String::new(),
        }
    }

    /// A per-invariant label.
    #[must_use]
    pub fn named(stage: Stage, name: &str) -> Self {
        Self {
            stage,
            name: name.to_owned(),
        }
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.name.is_empty() {
            f.write_str(self.stage.token())
        } else {
            write!(f, "{}:{}", self.stage.token(), self.name)
        }
    }
}

/// The only epoch this spike's definitions consume. RFC 0030, "Which epochs enter
/// the key": every semantic query consumes the semantic epoch; none of these
/// definitions reads a proof, intent, evidence, or corpus epoch, and the protocol
/// epoch never enters a key.
pub const DEFAULT_SEMANTIC_EPOCH: &str = "continuum-semantics-1";

/// One declared dependency edge of a definition: where it reads from, why, and the
/// class a reuse through it can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeSpec {
    /// The stage read.
    pub from: Stage,
    /// The typed reason.
    pub reason: DependencyReason,
    /// The class a reuse carries through this edge when the key does not name what
    /// the edge reads (a *side input*). A keyed edge is `Exact`.
    pub class: ReuseClass,
    /// Whether the key names this input's identity.
    pub keyed: Keyed,
    /// The assumption a `Conservative` edge rests on, recorded on the edge and
    /// audited.
    pub assumption: &'static str,
}

/// Whether an edge's input identity is part of the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyed {
    /// The key names the input's output identity.
    Identity,
    /// The key does not name the input. A reuse through this edge whose input
    /// changed is licensed by the edge's class and assumption.
    Side,
}

/// A registered query definition (RFC 0030, "Query definition registry").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryDefinition {
    /// The stage.
    pub stage: Stage,
    /// The registry name (lowercase, `snake_case`, verb-first).
    pub function_id: &'static str,
    /// The implementation version. Advanced whenever an output could change.
    pub function_version: &'static str,
    /// The auditability class, declared once on the definition.
    pub auditability: Auditability,
    /// Whether the result is budget-sensitive. No definition here is: the explore
    /// bounds are declared scope configuration (`strategy_config`), and the result
    /// is a deterministic function of the key under them.
    pub budget_sensitive: bool,
    /// The declared dependency edges, in the order explanations walk them.
    pub edges: &'static [EdgeSpec],
}

const fn keyed(from: Stage, reason: DependencyReason) -> EdgeSpec {
    EdgeSpec {
        from,
        reason,
        class: ReuseClass::Exact,
        keyed: Keyed::Identity,
        assumption: "",
    }
}

/// Assumption A1 of the `Conservative` edges into `explore` and `domain_safety`.
pub const ASSUMPTION_A1: &str = "A1: exploration reads the variables, initial states and actions of the \
     lowered model and never its predicates, except the actions' definedness predicates, \
     which are lowered from the action declarations alone and so are covered by the \
     transition-system projection; so, among models that lower (the query is \
     demanded only when build_model succeeds), an edit that leaves the \
     transition-system projection unchanged leaves the exploration unchanged";

/// The assumption a `Validated` reuse of `domain_safety` records: the certificate the
/// kernel verified covers closure within the state domain only.
pub const ASSUMPTION_V1: &str = "V1: action definedness over the witness set is evaluated by the \
     engine's model evaluator (the same evaluator the clean run uses), not by the kernel; \
     the certificate establishes closure within the state domain only";

/// Assumption A2 of the `Conservative` edge into `check_invariant`.
pub const ASSUMPTION_A2: &str = "A2: the lowered predicate of one invariant and its definedness \
     predicate are functions of that invariant's clauses and of the transition-system \
     declarations only, and the actions' definedness predicates of the transition-system \
     declarations only; never of the other invariants or of the invariant's name";

/// The registry. Closed and ordered by stage.
pub const REGISTRY: &[QueryDefinition] = &[
    QueryDefinition {
        stage: Stage::Parse,
        function_id: "parse",
        function_version: "1",
        auditability: Auditability::EqualityAuditable,
        budget_sensitive: false,
        edges: &[keyed(Stage::Source, DependencyReason::ReadsValue)],
    },
    QueryDefinition {
        stage: Stage::Elaborate,
        function_id: "elaborate",
        function_version: "1",
        auditability: Auditability::EqualityAuditable,
        budget_sensitive: false,
        edges: &[keyed(Stage::Parse, DependencyReason::ReadsValue)],
    },
    QueryDefinition {
        stage: Stage::BuildModel,
        function_id: "build_model",
        function_version: "1",
        auditability: Auditability::EqualityAuditable,
        budget_sensitive: false,
        edges: &[keyed(Stage::Elaborate, DependencyReason::ReadsValue)],
    },
    QueryDefinition {
        stage: Stage::ProjectTransitionSystem,
        function_id: "project_transition_system",
        function_version: "1",
        auditability: Auditability::EqualityAuditable,
        budget_sensitive: false,
        edges: &[keyed(Stage::Elaborate, DependencyReason::ReadsValue)],
    },
    QueryDefinition {
        stage: Stage::ProjectInvariant,
        function_id: "project_invariant",
        function_version: "1",
        auditability: Auditability::EqualityAuditable,
        budget_sensitive: false,
        edges: &[keyed(Stage::Elaborate, DependencyReason::ReadsValue)],
    },
    QueryDefinition {
        stage: Stage::Explore,
        function_id: "explore",
        function_version: "1",
        auditability: Auditability::EqualityAuditable,
        budget_sensitive: false,
        edges: &[
            keyed(Stage::ProjectTransitionSystem, DependencyReason::ReadsValue),
            EdgeSpec {
                from: Stage::BuildModel,
                reason: DependencyReason::ReadsValue,
                class: ReuseClass::Conservative,
                keyed: Keyed::Side,
                assumption: ASSUMPTION_A1,
            },
        ],
    },
    QueryDefinition {
        stage: Stage::DomainSafety,
        function_id: "domain_safety",
        // 2 (cr-1jv75r): keyed on the exploration, and undefined action reads.
        function_version: "2",
        auditability: Auditability::EqualityAuditable,
        budget_sensitive: false,
        edges: &[
            keyed(Stage::ProjectTransitionSystem, DependencyReason::ReadsValue),
            // The verdict is computed from the exploration, so the exploration's
            // identity is keyed and its class and provenance reach the result.
            keyed(Stage::Explore, DependencyReason::ReadsValue),
            EdgeSpec {
                from: Stage::BuildModel,
                reason: DependencyReason::ReadsValue,
                class: ReuseClass::Conservative,
                keyed: Keyed::Side,
                assumption: ASSUMPTION_A1,
            },
        ],
    },
    QueryDefinition {
        stage: Stage::CheckInvariant,
        function_id: "check_invariant",
        // 2 (cr-1jv75r): undefined invariant and action reads are typed outcomes.
        function_version: "2",
        auditability: Auditability::EqualityAuditable,
        budget_sensitive: false,
        edges: &[
            keyed(Stage::ProjectInvariant, DependencyReason::ReadsValue),
            keyed(Stage::ProjectTransitionSystem, DependencyReason::ReadsType),
            keyed(Stage::Explore, DependencyReason::ReadsValue),
            EdgeSpec {
                from: Stage::BuildModel,
                reason: DependencyReason::ReadsValue,
                class: ReuseClass::Conservative,
                keyed: Keyed::Side,
                assumption: ASSUMPTION_A2,
            },
        ],
    },
];

/// A quarantine unit: `(reuse-edge class, function_id, function_version)` (RFC 0030,
/// "Quarantine").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Triple {
    /// The class of the licence that was used.
    pub class: ReuseClass,
    /// The definition.
    pub function_id: &'static str,
    /// Its version.
    pub function_version: &'static str,
}

impl fmt::Display for Triple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({}, {}, {})",
            self.class, self.function_id, self.function_version
        )
    }
}

/// A canonical query key (RFC 0030, "Query key"): `function_id`, `function_version`,
/// the ordered canonical input identities, the consumed epochs, and the strategy
/// configuration. Two keys are equal iff their encodings are byte-equal, and the
/// encoding is length-prefixed throughout, so one key has one spelling.
///
/// Each input identity is a *content commitment*: the BLAKE3 digest of the upstream
/// canonical output (RFC 0030 admits "a plan §4.4 handle or a content commitment").
/// A commitment keeps every key a constant size, so a model with `n` invariants
/// costs `n` small keys rather than `n` copies of the normalized model. The
/// commitment is an index, not the identity relation: a cache entry also holds the
/// full canonical inputs, and a hit is admitted only after they compare equal byte
/// for byte (ADR-0013: a collision is resolved by canonical comparison, never
/// trusted).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueryKey {
    bytes: Vec<u8>,
}

impl QueryKey {
    /// The key tag.
    pub const TAG: &'static [u8] = b"continuum-incremental-key/1";

    /// The encoded size of a key, computed without building it, so the caller can
    /// charge it before the allocation.
    #[must_use]
    pub fn encoded_len(
        definition: &QueryDefinition,
        name: &str,
        inputs: &[Digest256],
        semantic_epoch: &str,
        strategy: &[u8],
    ) -> usize {
        let prefixed = |n: usize| n.saturating_add(8);
        let mut total = Self::TAG.len();
        total = total.saturating_add(prefixed(definition.function_id.len()));
        total = total.saturating_add(prefixed(definition.function_version.len()));
        total = total.saturating_add(prefixed(name.len()));
        total = total.saturating_add(8);
        total = total.saturating_add(inputs.len().saturating_mul(prefixed(DIGEST_LEN)));
        total = total.saturating_add(prefixed(b"semantic".len()));
        total = total.saturating_add(prefixed(semantic_epoch.len()));
        total.saturating_add(prefixed(strategy.len()))
    }

    /// Build a key. `name` is part of the key only as the input selector of
    /// `project_invariant`; the per-invariant check is keyed by content, so a
    /// renamed invariant with an unchanged body reuses its result.
    #[must_use]
    pub fn new(
        definition: &QueryDefinition,
        name: &str,
        inputs: &[Digest256],
        semantic_epoch: &str,
        strategy: &[u8],
    ) -> Self {
        let mut bytes = Vec::with_capacity(Self::encoded_len(
            definition,
            name,
            inputs,
            semantic_epoch,
            strategy,
        ));
        bytes.extend_from_slice(Self::TAG);
        field(&mut bytes, definition.function_id.as_bytes());
        field(&mut bytes, definition.function_version.as_bytes());
        field(&mut bytes, name.as_bytes());
        bytes.extend_from_slice(&(inputs.len() as u64).to_be_bytes());
        for input in inputs {
            field(&mut bytes, input.as_bytes());
        }
        // `consumed_epochs`: the declared subset, pinned. Every other epoch is
        // unpinned by omission from this closed, single-member encoding.
        field(&mut bytes, b"semantic");
        field(&mut bytes, semantic_epoch.as_bytes());
        field(&mut bytes, strategy);
        Self { bytes }
    }

    /// The canonical encoding.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The display handle: `q_` and the BLAKE3 token of the canonical key. A handle
    /// indexes; it never decides equality.
    #[must_use]
    pub fn handle(&self) -> String {
        format!("q_{}", Blake3Hasher::hash(&self.bytes).to_token())
    }

    /// The display handle of one query instance: the key and the label together.
    /// Content addressing lets two labels share one key (a renamed invariant, or two
    /// invariants with one body), and an explanation must name each query once, so
    /// the handle a report carries is per label. The label is length-prefixed after
    /// the key, so the pair has one spelling.
    #[must_use]
    pub fn handle_for(&self, label: &Label) -> String {
        let mut bytes = self.bytes.clone();
        field(&mut bytes, label.stage.token().as_bytes());
        field(&mut bytes, label.name.as_bytes());
        format!("q_{}", Blake3Hasher::hash(&bytes).to_token())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stage_but_source_has_exactly_one_definition() {
        for stage in [
            Stage::Parse,
            Stage::Elaborate,
            Stage::BuildModel,
            Stage::ProjectTransitionSystem,
            Stage::ProjectInvariant,
            Stage::Explore,
            Stage::DomainSafety,
            Stage::CheckInvariant,
        ] {
            let count = REGISTRY.iter().filter(|d| d.stage == stage).count();
            assert_eq!(count, 1, "{stage:?}");
        }
        assert!(Stage::Source.definition().is_none());
        // Edges point strictly upstream, so explanations terminate.
        for definition in REGISTRY {
            for edge in definition.edges {
                assert!(edge.from < definition.stage, "{}", definition.function_id);
                if edge.class == ReuseClass::Conservative {
                    assert!(!edge.assumption.is_empty());
                }
            }
        }
    }

    #[test]
    fn keys_are_injective_over_field_boundaries() {
        let d = Stage::Parse.definition().expect("registered");
        let x = Blake3Hasher::hash(b"x");
        let y = Blake3Hasher::hash(b"y");
        let a = QueryKey::new(d, "", &[x, y], "e", b"");
        let b = QueryKey::new(d, "", &[y, x], "e", b"");
        let c = QueryKey::new(d, "", &[x], "e", b"");
        let e = QueryKey::new(d, "", &[x, y], "f", b"");
        let n = QueryKey::new(d, "ab", &[x, y], "e", b"");
        let m = QueryKey::new(d, "a", &[x, y], "e", b"b");
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, e);
        assert_ne!(n, m);
        assert_eq!(a, QueryKey::new(d, "", &[x, y], "e", b""));
        assert_eq!(
            a.as_bytes().len(),
            QueryKey::encoded_len(d, "", &[x, y], "e", b"")
        );
    }
}
