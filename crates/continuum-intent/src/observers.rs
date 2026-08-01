//! The `observers` field group: what a claim is allowed to see (PR-4 / IMPL-03).
//!
//! # What an observer *is*, in this crate
//!
//! RFC 0037's field table gives the shape and RFC 0031 gives the unit of
//! classification, and together they say an observer is a **unit key and four
//! sets**:
//!
//! > | `observers[]` | `{id, events, state_projection?, knowledge_projection?,
//! > security_projection?}` | `id`, `events` | event families plus the four
//! > projection kinds of plan §5.2; all are string sets with `uniqueItems`; the unit
//! > reductions are justified against the observer (INV-013) |
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`, "Field types and enums"
//!
//! > Keyed by `id`. Compare the four sets `events`, `state_projection`,
//! > `knowledge_projection`, `security_projection` componentwise. All four supersets,
//! > at least one strict, is `refined`; all four subsets, at least one strict, is
//! > `coarsened`; equal on all four is `unchanged`; any mixed movement is
//! > `incomparable`. Dropping an event family or a projection element is the "hide
//! > observer events" attack (plan §19.5) and MUST classify `coarsened` even when
//! > other components grow.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "`observers`"
//!
//! [`Observer`] is exactly that: an [`ObserverId`] and one set per
//! [`ProjectionKind`]. The four components are stored *uniformly* — the required
//! `events` set is not a privileged field with three projections beside it — because
//! the comparison RFC 0031 performs is componentwise over all four, and a
//! representation that made one of them structurally different would invite a
//! classifier that treated it differently.
//!
//! # The protected weakening this shape makes visible
//!
//! "Coarsen observer" is one of the intent attacks
//! `notes/plan/docs/50_AGENT_EVALUATION_AND_REWARD_HACKING.md` lists, and RFC 0031's
//! attack table gives it a verdict: *change `Agreement` to a weaker observer →
//! `observers` → `coarsened` → `review`*. The move at this layer is to drop one
//! string from one set and hope the drop is not surfaced.
//!
//! Three properties of this module make that structurally impossible to hide:
//!
//! 1. **Set membership is the comparison surface.** [`Observer::components`] hands a
//!    classifier the four sets directly, in a fixed order, with absence already
//!    normalized to the empty set. There is no encoding in which "the projection is
//!    missing" and "the projection is empty" differ, so a dropped *key* is exactly as
//!    visible as a dropped *element* — which is what RFC 0037's absence rule demands
//!    ("so that 'absent' and 'empty' cannot yield two identities for one meaning").
//! 2. **The unit key is stable and independent of content.** An observer is keyed by
//!    `id`, so an edit to its sets is an edit to *that* unit, never a `removed` plus
//!    an `added` that a classifier might pair up differently.
//! 3. **The identity is the canonical bytes of all four sets.** Dropping one element
//!    changes [`Observer::identity`], and ID7 requires exactly that: "Changing any
//!    claim, assumption, observer, bound, fault, fairness constraint […] MUST change
//!    it."
//!
//! # One spelling, not two
//!
//! [`crate::property`] carries two byte spellings — the artifact form and the ID2
//! identity preimage — because two members of ID2's closed exclusion list
//! (`expression.source`, `expression.normal_form`) live inside a claim. No member of
//! that list has an admissible site inside `observers[]`: there is no `name`, no
//! `source`, no `normal_form`, and `additionalProperties: false` on the item leaves
//! `$comment` nowhere to sit. So for this group the artifact form *is* the identity
//! preimage, [`Observer::to_artifact_bytes`] and [`Observer::identity`] agree byte for
//! byte, and there is no layer parameter to get wrong.
//!
//! # Ordering, and what is not a change
//!
//! `observers` is a keyed array, so [`ObserverSet`] stores it as a map and encodes in
//! `id` order; the elements of each set are encoded in Unicode code-point order,
//! which is ID5's order for object keys applied to a `uniqueItems` array. Reordering
//! the authored array or the authored elements is therefore a reformatting and does
//! not move the identity, which is what ID7 requires of a reformat. *This repeats the
//! flag [`crate::property::ClaimSet`] raises against RFC 0037: ID5 fixes the order of
//! object keys and says nothing about arrays, and every group with a declared unit
//! key or a `uniqueItems` set needs the same answer.*
//!
//! # Seams left open on purpose
//!
//! - **W2 is closed from this side by [`ObserverSet::declared_ids`]**, which produces
//!   exactly the argument [`crate::property::ClaimSet::check_observers`] takes. The
//!   caller that holds both groups is the contract-assembly bone's.
//! - **INV-013 — "the unit reductions are justified against the observer" — is not
//!   here.** It is a statement about a verification run, not about the contract, and
//!   this crate is in the smallest trust base.
//! - **Classification is not here.** Whether a change is `refined`, `coarsened`, or
//!   `incomparable` is RFC 0031's question (PR 12). This module supplies the sets it
//!   compares and the identity S1 permits `unchanged` to be claimed on.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use crate::canonical_json::{Json, JsonError};
use crate::identity::canonical_identity;

/// The classified-unit key of an observer: `observers[].id`.
///
/// RFC 0031 keys the `observers` field's classification by exactly this, and
/// `semantic-diff.schema.json` renders it as the per-unit `unit` locator with
/// `minLength: 1` ("Claims, assumptions, and observers are keyed by `id`"). An empty
/// key is rejected for that reason: a unit locator that names nothing cannot appear
/// in a diff record.
///
/// [`Ord`] is byte order over UTF-8, which is ID5's code-point order, so
/// [`ObserverSet`]'s encoding is stable and its diff keying deterministic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObserverId(String);

impl ObserverId {
    /// Validate and wrap an observer id.
    ///
    /// # Errors
    ///
    /// [`ObserverError::EmptyObserverId`] for the empty string.
    pub fn new(id: &str) -> Result<Self, ObserverError> {
        if id.is_empty() {
            return Err(ObserverError::EmptyObserverId);
        }
        Ok(Self(id.to_owned()))
    }

    /// The key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObserverId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One of the four sets RFC 0031 compares componentwise.
///
/// The vocabulary is closed. Plan §5.2 lists an observer's projections as
/// "state/event/knowledge/security", and `intent-contract.schema.json` spells them as
/// four keys of the `observers[]` item — `events` being the event projection, which
/// the schema makes required while the other three are optional and default to the
/// empty set. *Raised as a flag against RFC 0037: its field table reads "event
/// families plus the four projection kinds", which counts five; plan §5.2 and the
/// schema agree on four, of which `events` is one, and RFC 0031's comparison rule
/// says "the four sets `events`, `state_projection`, `knowledge_projection`,
/// `security_projection`". The RFC's wording should be corrected to agree.*
///
/// This is the group's *only* closed vocabulary, and it is realized as object keys
/// rather than as values, so an unrecognized member is rejected as
/// [`ObserverDecodeError::UnknownField`] under `additionalProperties: false` — the
/// same fail-closed answer, reported where the schema states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectionKind {
    /// `events` — the event families the observer sees. Required by the schema.
    Events,
    /// `state_projection` — the state components the observer can read.
    State,
    /// `knowledge_projection` — what the observer may come to know.
    Knowledge,
    /// `security_projection` — the security-relevant view.
    Security,
}

impl ProjectionKind {
    /// Every projection, in the order the canonical encoding and every componentwise
    /// comparison walk them.
    pub const ALL: [Self; 4] = [Self::Events, Self::State, Self::Knowledge, Self::Security];

    /// The wire key.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Events => "events",
            Self::State => "state_projection",
            Self::Knowledge => "knowledge_projection",
            Self::Security => "security_projection",
        }
    }

    /// Recover a projection from its wire key.
    ///
    /// Returns `None` for an unrecognized key. The caller rejects: "Forward
    /// compatibility is achieved by rejecting, never by ignoring" (RFC 0037).
    #[must_use]
    pub fn from_wire(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.wire() == key)
    }

    /// Whether the schema requires this projection to be present in a document.
    ///
    /// Only `events` is. The distinction is a *document* fact and not a semantic one:
    /// an absent optional projection reads as the empty set, and this module encodes
    /// all four explicitly, so nothing downstream can tell an absent projection from
    /// an empty one.
    #[must_use]
    pub const fn is_required(self) -> bool {
        matches!(self, Self::Events)
    }

    const fn index(self) -> usize {
        match self {
            Self::Events => 0,
            Self::State => 1,
            Self::Knowledge => 2,
            Self::Security => 3,
        }
    }
}

impl fmt::Display for ProjectionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

canonical_identity! {
    /// The canonical identity of an observer artifact.
    ///
    /// The discipline is `crate::identity::CanonicalIdentity`'s, stated in full there
    /// and applied here: the identity *is* the canonical preimage bytes, not a digest
    /// of them, so equality is canonical comparison (ADR-0013) and
    /// [`ObserverIdentity::digest`] can only index. *The seam this type's doc comment
    /// used to name — "when a later bone gives the crate one shared identity newtype,
    /// this type and its siblings […] collapse into it" — is closed: all nine field
    /// groups are now newtypes over that one core. They stay nine distinct types so
    /// that an observer's identity does not typecheck where a claim's is expected.*
    ObserverIdentity
}

/// One observer: `observers[]`.
///
/// Immutable. There is no method taking `&mut self` and no constructor that reuses an
/// identity, for the reason `lib.rs` states: a setter is the affordance by which an
/// ordinary operation mutates intent, and that is the single thing INV-001 forbids. A
/// coarsened observer is a *new* [`Observer`] with a new [`ObserverIdentity`].
///
/// [`PartialEq`] is hand-written and compares the id and the four sets. It never
/// consults [`identity`](Self::identity), so a future identity that is not a pure
/// function of content cannot quietly become the equality relation.
#[derive(Debug, Clone)]
pub struct Observer {
    id: ObserverId,
    components: [BTreeSet<String>; 4],
    identity: ObserverIdentity,
}

impl Observer {
    /// Build an observer from its id and its projections.
    ///
    /// Projections are supplied by [`ProjectionKind`] rather than positionally: the
    /// four sets are interchangeable in type, and a positional constructor would make
    /// "swap `state_projection` and `security_projection`" a silent mistake. A kind
    /// that is not supplied is the empty set, which is what RFC 0037's absence rule
    /// says an absent optional projection means.
    ///
    /// Elements are *not* required to be non-empty strings: the schema states no
    /// `minLength` for them, and RFC 0037 is explicit that "a checker MUST NOT invent
    /// a value for a key the schema leaves optional". The id is different — it is the
    /// diff's unit locator, which the diff schema does bound.
    ///
    /// # Errors
    ///
    /// - [`ObserverError::DuplicateProjection`] — the same kind supplied twice, which
    ///   has no single reading.
    /// - [`ObserverError::DuplicateElement`] — a repeated element, which the schema's
    ///   `uniqueItems: true` forbids. It is rejected rather than deduplicated: a
    ///   reader that silently collapsed it would accept documents the schema rejects.
    pub fn new(
        id: ObserverId,
        projections: impl IntoIterator<Item = (ProjectionKind, Vec<String>)>,
    ) -> Result<Self, ObserverError> {
        let mut supplied: [Option<BTreeSet<String>>; 4] = [None, None, None, None];
        for (kind, elements) in projections {
            if supplied[kind.index()].is_some() {
                return Err(ObserverError::DuplicateProjection { kind });
            }
            let mut set = BTreeSet::new();
            for element in elements {
                if !set.insert(element.clone()) {
                    return Err(ObserverError::DuplicateElement {
                        kind,
                        value: element,
                    });
                }
            }
            supplied[kind.index()] = Some(set);
        }
        let components = supplied.map(Option::unwrap_or_default);
        let identity =
            ObserverIdentity::of_bytes(observer_json(&id, &components).to_canonical_bytes());
        Ok(Self {
            id,
            components,
            identity,
        })
    }

    /// The classified-unit key.
    #[must_use]
    pub const fn id(&self) -> &ObserverId {
        &self.id
    }

    /// The RFC 0031 per-unit locator: an observer is "keyed by `id`".
    #[must_use]
    pub fn locator(&self) -> &str {
        self.id.as_str()
    }

    /// One projection. An unsupplied projection is the empty set, never absent.
    #[must_use]
    pub fn projection(&self, kind: ProjectionKind) -> &BTreeSet<String> {
        &self.components[kind.index()]
    }

    /// The event families the observer sees: the schema's required `events`.
    #[must_use]
    pub fn events(&self) -> &BTreeSet<String> {
        self.projection(ProjectionKind::Events)
    }

    /// The four sets RFC 0031 compares componentwise, in [`ProjectionKind::ALL`]
    /// order.
    ///
    /// This is the group's comparison surface. A classifier asks for the four sets and
    /// gets four sets — not an `Option`, not a document, and not a rendering it would
    /// have to re-parse.
    #[must_use]
    pub fn components(&self) -> [(ProjectionKind, &BTreeSet<String>); 4] {
        ProjectionKind::ALL.map(|kind| (kind, self.projection(kind)))
    }

    /// The observer's canonical identity, which is also its artifact bytes.
    #[must_use]
    pub const fn identity(&self) -> &ObserverIdentity {
        &self.identity
    }

    /// The artifact-form JSON: what a contract document carries.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        observer_json(&self.id, &self.components)
    }

    /// The artifact-form bytes, under the ID5 canonical-JSON rules.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// Decode an observer from its artifact form.
    ///
    /// # Errors
    ///
    /// [`ObserverDecodeError`], naming the field or the key that failed.
    pub fn decode(bytes: &[u8]) -> Result<Self, ObserverDecodeError> {
        Self::from_json(&Json::parse(bytes)?)
    }

    /// Decode an observer from an already-parsed JSON value.
    ///
    /// # Errors
    ///
    /// [`ObserverDecodeError`], as [`Observer::decode`].
    pub fn from_json(json: &Json) -> Result<Self, ObserverDecodeError> {
        let fields = json
            .as_object()
            .ok_or_else(|| ObserverDecodeError::TypeMismatch {
                field: "observers[]",
                expected: "object",
                found: json.type_name(),
            })?;
        for key in fields.keys() {
            if key != "id" && ProjectionKind::from_wire(key).is_none() {
                return Err(ObserverDecodeError::UnknownField {
                    field: "observers[]",
                    key: key.clone(),
                });
            }
        }
        let id_json = fields.get("id").ok_or(ObserverDecodeError::MissingField {
            field: "observers[].id",
        })?;
        let id = ObserverId::new(id_json.as_str().ok_or_else(|| {
            ObserverDecodeError::TypeMismatch {
                field: "observers[].id",
                expected: "string",
                found: id_json.type_name(),
            }
        })?)?;
        let mut projections = Vec::with_capacity(4);
        for kind in ProjectionKind::ALL {
            match fields.get(kind.wire()) {
                None if kind.is_required() => {
                    return Err(ObserverDecodeError::MissingField { field: kind.wire() });
                }
                // An absent optional set reads as the empty set, and the encoder
                // writes it back explicitly (RFC 0037, "Absence, null, and defaults").
                None => projections.push((kind, Vec::new())),
                Some(Json::Array(items)) => {
                    let mut elements = Vec::with_capacity(items.len());
                    for item in items {
                        elements.push(
                            item.as_str()
                                .ok_or_else(|| ObserverDecodeError::TypeMismatch {
                                    field: kind.wire(),
                                    expected: "string",
                                    found: item.type_name(),
                                })?
                                .to_owned(),
                        );
                    }
                    projections.push((kind, elements));
                }
                Some(other) => {
                    return Err(ObserverDecodeError::TypeMismatch {
                        field: kind.wire(),
                        expected: "array",
                        found: other.type_name(),
                    });
                }
            }
        }
        Ok(Self::new(id, projections)?)
    }
}

impl PartialEq for Observer {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.components == other.components
    }
}

impl Eq for Observer {}

fn observer_json(id: &ObserverId, components: &[BTreeSet<String>; 4]) -> Json {
    let mut fields = vec![("id".to_owned(), Json::String(id.as_str().to_owned()))];
    for kind in ProjectionKind::ALL {
        fields.push((
            kind.wire().to_owned(),
            Json::Array(
                components[kind.index()]
                    .iter()
                    .map(|element| Json::String(element.clone()))
                    .collect(),
            ),
        ));
    }
    Json::object(fields).unwrap_or(Json::Null)
}

/// The `observers` array: a set of observers keyed by their ids.
///
/// # Why a keyed set and not a list
///
/// W1 requires `observers[].id` to be unique — "duplicates make the diff's unit keying
/// ambiguous, which is a soundness hazard rather than a matter of taste" — and RFC
/// 0031 keys the `observers` classification by exactly that id. This type is that set:
/// [`ObserverSet::from_observers`] rejects a duplicate rather than keeping both, and
/// the encoding emits observers in id order.
///
/// The set MAY be empty, and an empty set is a statement: "An empty `assumptions`,
/// `observers`, `fairness`, `nondeterminism`, or `abstraction_maps` array is a
/// *declaration that the set is empty*, and is classified as such; it is not
/// 'unspecified'" (RFC 0037).
#[derive(Debug, Clone)]
pub struct ObserverSet {
    observers: BTreeMap<ObserverId, Observer>,
    identity: ObserverIdentity,
}

impl ObserverSet {
    /// Build an observer set, enforcing W1.
    ///
    /// # Errors
    ///
    /// [`ObserverError::DuplicateObserverId`] — two observers share an id. Two
    /// *byte-identical* entries are rejected too: a document that says the same thing
    /// twice is still a document whose unit keying is ambiguous.
    pub fn from_observers(
        observers: impl IntoIterator<Item = Observer>,
    ) -> Result<Self, ObserverError> {
        let mut keyed: BTreeMap<ObserverId, Observer> = BTreeMap::new();
        for observer in observers {
            if let Some(existing) = keyed.insert(observer.id().clone(), observer) {
                return Err(ObserverError::DuplicateObserverId {
                    id: existing.id().to_string(),
                });
            }
        }
        let identity = ObserverIdentity::of_bytes(
            Json::Array(keyed.values().map(Observer::artifact_json).collect()).to_canonical_bytes(),
        );
        Ok(Self {
            observers: keyed,
            identity,
        })
    }

    /// The `observers` portion of the contract's identity preimage.
    #[must_use]
    pub const fn identity(&self) -> &ObserverIdentity {
        &self.identity
    }

    /// How many observers the set carries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.observers.len()
    }

    /// Whether the contract declares that it binds no observer.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.observers.is_empty()
    }

    /// The observer under `id`, if any.
    #[must_use]
    pub fn get(&self, id: &ObserverId) -> Option<&Observer> {
        self.observers.get(id)
    }

    /// Every observer, in id order.
    pub fn iter(&self) -> impl Iterator<Item = &Observer> {
        self.observers.values()
    }

    /// Every unit key, in order: the membership half of RFC 0031's comparison.
    ///
    /// An id present on one side only classifies `added` or `removed`; an id present
    /// on both is classified componentwise by [`Observer::components`].
    #[must_use]
    pub fn unit_keys(&self) -> BTreeSet<&ObserverId> {
        self.observers.keys().collect()
    }

    /// The declared ids, in the shape [`crate::property::ClaimSet::check_observers`]
    /// takes.
    ///
    /// This closes W2 from the observer side: "A non-null `claims[].observer` MUST
    /// name an existing `observers[].id`. A dangling reference is rejected; it MUST
    /// NOT be treated as an unbound claim."
    #[must_use]
    pub fn declared_ids(&self) -> BTreeSet<String> {
        self.observers
            .keys()
            .map(|id| id.as_str().to_owned())
            .collect()
    }

    /// The artifact-form JSON array.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        Json::Array(
            self.observers
                .values()
                .map(Observer::artifact_json)
                .collect(),
        )
    }

    /// The artifact-form bytes.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// Decode an observer set from its artifact form.
    ///
    /// # Errors
    ///
    /// [`ObserverDecodeError`], including [`ObserverDecodeError::Observer`] for the W1
    /// violation the array shape carries.
    pub fn decode(bytes: &[u8]) -> Result<Self, ObserverDecodeError> {
        Self::from_json(&Json::parse(bytes)?)
    }

    /// Decode an observer set from an already-parsed JSON array.
    ///
    /// # Errors
    ///
    /// [`ObserverDecodeError`], as [`ObserverSet::decode`].
    pub fn from_json(json: &Json) -> Result<Self, ObserverDecodeError> {
        let items = json
            .as_array()
            .ok_or_else(|| ObserverDecodeError::TypeMismatch {
                field: "observers",
                expected: "array",
                found: json.type_name(),
            })?;
        let observers = items
            .iter()
            .map(Observer::from_json)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::from_observers(observers)?)
    }
}

impl PartialEq for ObserverSet {
    fn eq(&self, other: &Self) -> bool {
        self.observers == other.observers
    }
}

impl Eq for ObserverSet {}

impl<'a> IntoIterator for &'a ObserverSet {
    type Item = &'a Observer;
    type IntoIter = std::collections::btree_map::Values<'a, ObserverId, Observer>;

    fn into_iter(self) -> Self::IntoIter {
        self.observers.values()
    }
}

// --- errors --------------------------------------------------------------------------------

/// Why an observer could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserverError {
    /// An observer id is the empty string.
    EmptyObserverId,
    /// Two observers share an `observers[].id` (W1).
    DuplicateObserverId {
        /// The repeated key.
        id: String,
    },
    /// The same projection was supplied twice.
    DuplicateProjection {
        /// The repeated projection.
        kind: ProjectionKind,
    },
    /// A projection carries a repeated element, which `uniqueItems: true` forbids.
    DuplicateElement {
        /// The projection.
        kind: ProjectionKind,
        /// The repeated element.
        value: String,
    },
}

impl fmt::Display for ObserverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyObserverId => f.write_str(
                "an observer id is its classified-unit key (RFC 0031) and cannot be empty",
            ),
            Self::DuplicateObserverId { id } => write!(
                f,
                "two observers share the id {id:?}; W1 requires unique unit keys because the \
                 intent diff keys classification by them"
            ),
            Self::DuplicateProjection { kind } => write!(
                f,
                "the projection `{kind}` was supplied twice; an observer has one set per \
                 projection kind"
            ),
            Self::DuplicateElement { kind, value } => write!(
                f,
                "the projection `{kind}` repeats {value:?}; the schema sets uniqueItems: true, so \
                 a repeat is rejected rather than deduplicated"
            ),
        }
    }
}

impl core::error::Error for ObserverError {}

/// Why a byte string is not an observer artifact.
///
/// Every variant is a rejection, never a repair. RFC 0037: "an intent contract is
/// never best-effort decoded".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserverDecodeError {
    /// The bytes are not admissible canonical JSON.
    Json(JsonError),
    /// A required field is absent.
    MissingField {
        /// The field's path.
        field: &'static str,
    },
    /// A field carries the wrong JSON type.
    TypeMismatch {
        /// The field's path.
        field: &'static str,
        /// What the schema admits.
        expected: &'static str,
        /// What was found.
        found: &'static str,
    },
    /// An object carries a key the schema's `additionalProperties: false` forbids.
    ///
    /// This is also how the group's one closed vocabulary fails closed: the projection
    /// kinds are *keys*, so a fifth projection — or `state` written where
    /// `state_projection` was meant — arrives here rather than as an unknown token.
    UnknownField {
        /// The object's path.
        field: &'static str,
        /// The offending key.
        key: String,
    },
    /// The decoded observers do not form a well-formed set.
    Observer(ObserverError),
}

impl fmt::Display for ObserverDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => error.fmt(f),
            Self::MissingField { field } => write!(f, "the required field `{field}` is absent"),
            Self::TypeMismatch {
                field,
                expected,
                found,
            } => write!(f, "`{field}` must be {expected}; found {found}"),
            Self::UnknownField { field, key } => write!(
                f,
                "`{field}` carries the unknown key {key:?}; the schema sets \
                 additionalProperties: false and a reader that ignored it would accept documents \
                 the schema rejects — and, for an observer, would silently drop a projection"
            ),
            Self::Observer(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for ObserverDecodeError {}

impl From<JsonError> for ObserverDecodeError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

impl From<ObserverError> for ObserverDecodeError {
    fn from(error: ObserverError) -> Self {
        Self::Observer(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> ObserverId {
        ObserverId::new(text).expect("a test observer id is non-empty")
    }

    fn observer(text: &str, events: &[&str]) -> Observer {
        Observer::new(
            id(text),
            [(
                ProjectionKind::Events,
                events.iter().map(|event| (*event).to_owned()).collect(),
            )],
        )
        .expect("a test observer is well formed")
    }

    #[test]
    fn the_projection_vocabulary_is_closed_and_four_members_wide() {
        assert_eq!(ProjectionKind::ALL.len(), 4);
        for kind in ProjectionKind::ALL {
            assert_eq!(ProjectionKind::from_wire(kind.wire()), Some(kind));
        }
        // Plan §5.2's prose names, which are not the wire keys.
        assert_eq!(ProjectionKind::from_wire("state"), None);
        assert_eq!(ProjectionKind::from_wire("event"), None);
        assert_eq!(ProjectionKind::from_wire("Events"), None);
        assert!(ProjectionKind::Events.is_required());
        assert!(!ProjectionKind::State.is_required());
    }

    #[test]
    fn an_observer_id_cannot_be_empty() {
        assert_eq!(ObserverId::new(""), Err(ObserverError::EmptyObserverId));
        assert!(ObserverId::new("client").is_ok());
    }

    #[test]
    fn an_unsupplied_projection_is_the_empty_set_and_is_encoded_as_one() {
        let observer = observer("client", &["ReplyPublished"]);
        for kind in [
            ProjectionKind::State,
            ProjectionKind::Knowledge,
            ProjectionKind::Security,
        ] {
            assert!(observer.projection(kind).is_empty());
        }
        let encoded = String::from_utf8(observer.to_artifact_bytes()).expect("utf-8");
        assert!(encoded.contains(r#""state_projection":[]"#), "{encoded}");
        assert!(
            encoded.contains(r#""knowledge_projection":[]"#),
            "{encoded}"
        );
        assert!(encoded.contains(r#""security_projection":[]"#), "{encoded}");
    }

    #[test]
    fn the_identity_is_the_artifact_bytes_because_no_id2_member_lives_here() {
        let observer = observer("client", &["ReplyPublished"]);
        assert_eq!(
            observer.identity().canonical_bytes(),
            observer.to_artifact_bytes()
        );
    }

    #[test]
    fn a_repeated_element_is_rejected_rather_than_deduplicated() {
        assert_eq!(
            Observer::new(
                id("client"),
                [(ProjectionKind::Events, vec!["A".to_owned(), "A".to_owned()])]
            ),
            Err(ObserverError::DuplicateElement {
                kind: ProjectionKind::Events,
                value: "A".to_owned()
            })
        );
    }

    #[test]
    fn a_repeated_projection_has_no_single_reading() {
        assert_eq!(
            Observer::new(
                id("client"),
                [
                    (ProjectionKind::Events, vec!["A".to_owned()]),
                    (ProjectionKind::Events, vec!["B".to_owned()]),
                ]
            ),
            Err(ObserverError::DuplicateProjection {
                kind: ProjectionKind::Events
            })
        );
    }

    #[test]
    fn w1_is_enforced_on_the_set_and_an_empty_set_is_a_declaration() {
        assert_eq!(
            ObserverSet::from_observers([observer("client", &["A"]), observer("client", &["B"])]),
            Err(ObserverError::DuplicateObserverId {
                id: "client".to_owned()
            })
        );
        assert!(
            ObserverSet::from_observers([])
                .expect("an empty set is legal")
                .is_empty()
        );
    }

    #[test]
    fn the_components_surface_is_four_sets_in_a_fixed_order() {
        let observer = observer("client", &["ReplyPublished"]);
        let components = observer.components();
        assert_eq!(
            components.map(|(kind, _)| kind),
            ProjectionKind::ALL,
            "the comparison order is fixed"
        );
        assert_eq!(components[0].1.len(), 1);
        assert_eq!(observer.locator(), "client");
    }

    #[test]
    fn declared_ids_is_the_w2_argument() {
        let set = ObserverSet::from_observers([observer("a", &["X"]), observer("b", &["Y"])])
            .expect("a well-formed set");
        let declared = set.declared_ids();
        assert!(declared.contains("a") && declared.contains("b"));
        assert_eq!(declared.len(), 2);
    }
}
