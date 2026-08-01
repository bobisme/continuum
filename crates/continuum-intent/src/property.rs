//! The `properties` field group: claims, their expressions, and their identities
//! (PR-4 / IMPL-01).
//!
//! # What a property *is*, in this crate
//!
//! RFC 0037's field table gives the shape and RFC 0031 gives the unit of
//! classification, and together they say a property is four things:
//!
//! | Part | Type | Source |
//! |---|---|---|
//! | its **unit key** | [`UnitKey`] | RFC 0031: the `properties` field is classified "one claim, keyed by `id`", and `semantic-diff.schema.json`'s `unit` is that key |
//! | its **class** | [`ClaimKind`] | RFC 0037 correction 7: "a claim is `{id, kind, expression, observer?}` with `kind` from the closed six-member enum" |
//! | its **formula**, normalized | [`PropertyExpression`] | RFC 0037's CPNF-1: the AST in normal form, plus the fragment it is stated in and a display-only rendering |
//! | its **identity** | [`PropertyIdentity`] | ADR-0013 and RFC 0037 ID1/ID5: the canonical bytes of the semantic content |
//!
//! [`Claim`] is exactly that quadruple, and it is immutable: there is no `&mut`
//! method, no setter, and no `with_*` builder. RFC 0037 is explicit that a change is
//! a new artifact — "Accepted contracts are immutable. A revision never edits a
//! stored contract: it mints a new `in_*` and records a supersession edge" — so a
//! "changed claim" is a new [`Claim`] with a new [`PropertyIdentity`], and that is
//! the only spelling available.
//!
//! # The two byte spellings, and which layer owns each
//!
//! One artifact has one encoding, but a claim participates in *two* documents and
//! they are not the same bytes:
//!
//! - **The artifact form** ([`Claim::to_artifact_bytes`]) is what a contract
//!   document contains: the claim as `intent-contract.schema.json` admits it,
//!   including the display-only `expression.source` and the `expression.normal_form`
//!   declaration. It is written under the ID5 canonical-JSON rules, so even the
//!   display-only fields have exactly one spelling.
//! - **The identity preimage** ([`Claim::identity`]) is what the `in_*` identity is
//!   computed over. RFC 0037 ID2 fixes a closed exclusion list, and two of its five
//!   members live inside a claim:
//!
//!   > **ID2.** The exclusion list is **closed**. Exactly these bytes are excluded
//!   > from the preimage: `name`; `expression.source`; `expression.normal_form`;
//!   > every `$comment`; and `intent_id` itself […]
//!
//!   So the preimage of a claim is the artifact form minus `source` and
//!   `normal_form`. Nothing else is dropped: the `id`, the `kind`, the `observer`
//!   binding, the `fragment`, and the AST are all in.
//!
//! `$comment` is the third ID2 member and it has no admissible site here:
//! `additionalProperties: false` holds on `claims[]`, on `$defs/property_expression`,
//! and on every formula and term node, so a `$comment` inside the property portion
//! is a document the schema rejects and [`Claim::decode`] rejects it too (INV-003:
//! the schema decides shape). *Raised as a flag against RFC 0037: ID2's `$comment`
//! exclusion is unreachable within `claims`.*
//!
//! This is the whole of "which layer owns which spelling":
//!
//! - **RFC 0037 owns the byte spelling.** ID5 says JSON; there is no binary framing.
//! - **`intent-contract.schema.json` owns the document shape.** Every key this
//!   module writes or reads is one the schema names, and no key it does not.
//! - **ADR-0013 owns the identity discipline.** [`PropertyIdentity`] *is* the
//!   preimage bytes — not a digest of them — so equality is canonical comparison and
//!   a hash can only ever index.
//! - **`continuum-value` owns the hash seam.** [`PropertyIdentity::digest`] takes a
//!   `ContentHasher`, so this crate makes no hash-vendor decision and opens no second
//!   seam.
//!
//! # Absence, null, and the two identities one meaning must not have
//!
//! RFC 0037: an absent optional set "MUST be encoded explicitly as the empty set
//! when the identity preimage is built (ID5), so that 'absent' and 'empty' cannot
//! yield two identities for one meaning". The property portion has three places
//! where absence could otherwise fork, and each is settled here:
//!
//! - **`args` / `indices`** — always encoded, empty included ([`crate::ast`]).
//! - **`claims[].observer`** — always encoded, `null` when unbound. The RFC says the
//!   field is "a string or `null` for a claim stated without an observer binding"
//!   and its absence/null table does not name the key, so absent and `null` are read
//!   as one meaning with one spelling. *Raised as a flag against RFC 0037: the
//!   absence/null section should name `claims[].observer`.*
//! - **`expression.fragment`** — encoded only when present, because here absence
//!   *is* a distinct meaning: W3 makes an absent `fragment` denote the intent's
//!   declared fragment, which is a statement about the contract, not about the
//!   expression. Normalizing it away would silently bind an expression to a fragment
//!   nobody wrote.
//!
//! # Seams left open on purpose
//!
//! - **The other fourteen field groups.** `assumptions` reuses
//!   [`PropertyExpression`] unchanged — RFC 0037 gives `claims[].expression` and
//!   `assumptions[].expression` the same `property_expression` shape — and
//!   `fairness[].condition` is a *bare* temporal-free formula, for which
//!   [`crate::ast::Formula::is_temporal_free`] is the W4 check. Neither group is
//!   implemented here.
//! - **The reader is not here either.** The recursive descent over
//!   `$defs/property_expression` and the formula/term nodes beneath it lives in
//!   `crate::codec`, the crate's one reader for that grammar, and is entered from
//!   here through [`PropertyExpression::from_json`]. It was private to this module
//!   while the PR-4 field-group bones were in flight, which cost `assumptions` a
//!   second copy of it and `fairness` a synthetic-claim wrapper to reach it; both
//!   are gone. What stays here is what is the `claims[]` item's own: its four keys,
//!   its two byte spellings, and its identity.
//! - **W2 and W3 need the rest of the contract.** [`ClaimSet::observer_references`]
//!   and [`ClaimSet::check_fragments`] are the property-side halves; the observer
//!   ids and `scope.fragments` come from sibling groups, so the callers are the
//!   contract-assembly bone's.
//! - **The contract identity is not here.** `in_*` is computed over the whole
//!   document (ID1), of which this is one field. This module supplies the `claims`
//!   portion of that preimage through [`ClaimSet::identity`] and nothing more.
//! - **Classification is not here.** Whether a change is `weakened` or `unknown` is
//!   RFC 0031's question (PR 12). What this module guarantees is the only thing S1
//!   permits `unchanged` to be claimed on: equal canonical bytes.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{AstError, Formula, Fragment};
use crate::canonical_json::{Json, JsonError};
use crate::codec::{known_keys, object, string_field};
use crate::cpnf::{self, CPNF_VERSION, NormalizeError};
use crate::identity::canonical_identity;

/// The classified-unit key of a claim: `claims[].id`.
///
/// RFC 0031 keys the `properties` field's classification by exactly this, and
/// `semantic-diff.schema.json`'s per-unit `unit` locator carries it with
/// `minLength: 1`. An empty key is rejected here for that reason: a unit locator
/// that names nothing cannot appear in a diff record, and W1's uniqueness rule would
/// then be enforced over a key with no referent.
///
/// [`Ord`] is byte order over UTF-8, which is ID5's code-point order. That is what
/// makes [`ClaimSet`]'s encoding stable and its diff keying deterministic (RFC 0031
/// requires "identical inputs yield byte-identical artifacts").
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitKey(String);

impl UnitKey {
    /// Validate and wrap a claim id.
    ///
    /// # Errors
    ///
    /// [`PropertyError::EmptyUnitKey`] for the empty string.
    pub fn new(id: &str) -> Result<Self, PropertyError> {
        if id.is_empty() {
            return Err(PropertyError::EmptyUnitKey);
        }
        Ok(Self(id.to_owned()))
    }

    /// The key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UnitKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a claim asserts: `claims[].kind`, the closed six-member enum.
///
/// RFC 0037 correction 7 makes this required and normative, and gives it teeth:
/// "a `kind` change with an unchanged expression classifies `incomparable`". So the
/// class is in the identity preimage, and changing it mints a new claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClaimKind {
    /// Nothing bad happens: an invariant over states.
    Safety,
    /// Something good eventually happens: a temporal obligation.
    Liveness,
    /// One system implements another.
    Refinement,
    /// A property of sets of behaviors rather than of one behavior.
    Hyperproperty,
    /// A security property.
    Security,
    /// A performance property.
    Performance,
}

impl ClaimKind {
    /// Every class, in schema-enum order.
    pub const ALL: [Self; 6] = [
        Self::Safety,
        Self::Liveness,
        Self::Refinement,
        Self::Hyperproperty,
        Self::Security,
        Self::Performance,
    ];

    /// The wire literal.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Safety => "safety",
            Self::Liveness => "liveness",
            Self::Refinement => "refinement",
            Self::Hyperproperty => "hyperproperty",
            Self::Security => "security",
            Self::Performance => "performance",
        }
    }

    /// Recover a class from its wire literal.
    ///
    /// Returns `None` for an unrecognized token. The caller rejects; RFC 0037 is
    /// explicit that forward compatibility "is achieved by rejecting, never by
    /// ignoring".
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.wire() == token)
    }
}

impl fmt::Display for ClaimKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// A structured property expression: `$defs/property_expression`.
///
/// The AST it holds is always in CPNF-1. [`PropertyExpression::normalized`] is the
/// only constructor, and it normalizes, so there is no path by which an
/// authoring-form AST reaches an identity. That is what lets
/// [`PropertyExpression::canonical_bytes`] be N8 by construction rather than by
/// discipline.
///
/// [`PartialEq`] is *document* equality and includes `source`. Identity is
/// *semantic* and excludes it (ID2). The two therefore differ exactly on the ID2
/// exclusion list, which is the distinction ID7 is built on: "rewriting `source`
/// […] MUST NOT change [the identity]".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyExpression {
    ast: Formula,
    fragment: Option<Fragment>,
    source: Option<String>,
}

impl PropertyExpression {
    /// Normalize `ast` to CPNF-1 and wrap it.
    ///
    /// `source` is retained verbatim and never consulted again: "It MUST NOT
    /// contribute to the `in_*` identity or to any RFC 0031 classification; where it
    /// disagrees with `ast`, `ast` is authoritative."
    ///
    /// # Errors
    ///
    /// [`PropertyError::Normalize`] when the AST is not one CPNF-1 can normalize.
    pub fn normalized(
        ast: &Formula,
        fragment: Option<Fragment>,
        source: Option<String>,
    ) -> Result<Self, PropertyError> {
        Ok(Self {
            ast: cpnf::normalize(ast)?,
            fragment,
            source,
        })
    }

    /// The CPNF-1 normal form of the expression.
    #[must_use]
    pub const fn ast(&self) -> &Formula {
        &self.ast
    }

    /// The declared fragment, or `None` when the intent's `scope.fragments` apply (W3).
    #[must_use]
    pub const fn fragment(&self) -> Option<Fragment> {
        self.fragment
    }

    /// The display-only surface rendering, if one was retained.
    #[must_use]
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// The identity-preimage JSON: the artifact form minus `source` and `normal_form`.
    #[must_use]
    pub fn identity_preimage_json(&self) -> Json {
        let mut fields = vec![("ast".to_owned(), self.ast.to_json())];
        if let Some(fragment) = self.fragment {
            fields.push((
                "fragment".to_owned(),
                Json::String(fragment.wire().to_owned()),
            ));
        }
        Json::object(fields).unwrap_or(Json::Null)
    }

    /// The artifact-form JSON: what a contract document carries.
    ///
    /// `normal_form` is always declared, because this type only ever holds a
    /// normalized AST. A checker "MUST verify the claim by re-normalizing and MUST
    /// reject a contract whose declaration does not hold" — [`Claim::decode`] does.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        let mut fields = vec![
            ("ast".to_owned(), self.ast.to_json()),
            (
                "normal_form".to_owned(),
                Json::String(CPNF_VERSION.to_owned()),
            ),
        ];
        if let Some(fragment) = self.fragment {
            fields.push((
                "fragment".to_owned(),
                Json::String(fragment.wire().to_owned()),
            ));
        }
        if let Some(source) = &self.source {
            fields.push(("source".to_owned(), Json::String(source.clone())));
        }
        Json::object(fields).unwrap_or(Json::Null)
    }

    /// The N8/ID5 bytes of the identity preimage.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        self.identity_preimage_json().to_canonical_bytes()
    }
}

canonical_identity! {
    /// The canonical identity of a property artifact.
    ///
    /// > Canonical structural encodings define identity. Hashes index and partition;
    /// > collisions resolve by exact comparison.
    /// >
    /// > — `notes/plan/adr/0013-exact-state-identity.md`, "Decision"
    ///
    /// This type *is* the identity-preimage bytes, exactly as
    /// `continuum_value::identity::ContentIdentity` is the CVNF-1 bytes: not a struct
    /// holding a digest, not a newtype over a hash. There is no hasher parameter and
    /// no field a hash could reach, so two properties are the same property precisely
    /// when their canonical encodings agree — a fact no hash-vendor decision can
    /// change. [`PropertyIdentity::digest`] exists for indexing and takes the hasher
    /// explicitly, through `continuum-value`'s existing seam.
    ///
    /// The bytes are UTF-8 by construction (ID5), so [`fmt::Display`] renders the
    /// identity as the canonical JSON itself. An identity that can be read is an
    /// identity a reviewer can check by hand, which is the point of ID7's checkable
    /// list.
    ///
    /// The discipline is `crate::identity::CanonicalIdentity`'s and is stated once
    /// there; this is the `properties` group's name for it, distinct from its eight
    /// siblings so that an identity of the wrong group does not typecheck.
    PropertyIdentity
}

/// One claim: `claims[]`.
///
/// Immutable. There is no method taking `&mut self`, and there is no constructor
/// that reuses an identity: [`Claim::new`] derives the identity from the content it
/// is given, so a claim in hand is a claim whose identity was computed from the
/// claim it actually is.
///
/// [`PartialEq`] is hand-written and compares the four declared parts. It never
/// consults [`identity`](Self::identity) — the identity is derived, so consulting it
/// would be comparing a value with itself the long way round, and the discipline
/// exists so that a future identity that is *not* a pure function of content cannot
/// quietly become the equality relation.
#[derive(Debug, Clone)]
pub struct Claim {
    unit: UnitKey,
    class: ClaimKind,
    expression: PropertyExpression,
    observer: Option<String>,
    identity: PropertyIdentity,
}

impl Claim {
    /// Build a claim and derive its identity.
    ///
    /// `observer` is a reference to an `observers[].id`, or `None` for "a claim
    /// stated without an observer binding". W2 — that a non-null reference resolves —
    /// needs the `observers` group and is [`ClaimSet::observer_references`]'s seam.
    #[must_use]
    pub fn new(
        unit: UnitKey,
        class: ClaimKind,
        expression: PropertyExpression,
        observer: Option<String>,
    ) -> Self {
        let identity = PropertyIdentity::of_bytes(
            claim_json(
                &unit,
                class,
                &expression,
                observer.as_deref(),
                Layer::Identity,
            )
            .to_canonical_bytes(),
        );
        Self {
            unit,
            class,
            expression,
            observer,
            identity,
        }
    }

    /// The classified-unit key.
    #[must_use]
    pub const fn unit(&self) -> &UnitKey {
        &self.unit
    }

    /// What the claim asserts.
    #[must_use]
    pub const fn class(&self) -> ClaimKind {
        self.class
    }

    /// The CPNF-1 expression.
    #[must_use]
    pub const fn expression(&self) -> &PropertyExpression {
        &self.expression
    }

    /// The bound observer, if any.
    #[must_use]
    pub fn observer(&self) -> Option<&str> {
        self.observer.as_deref()
    }

    /// The claim's canonical identity.
    #[must_use]
    pub const fn identity(&self) -> &PropertyIdentity {
        &self.identity
    }

    /// The artifact-form JSON: what a contract document carries.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        claim_json(
            &self.unit,
            self.class,
            &self.expression,
            self.observer.as_deref(),
            Layer::Artifact,
        )
    }

    /// The artifact-form bytes, under the ID5 canonical-JSON rules.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// Decode a claim from its artifact form.
    ///
    /// Strict in every direction the schema is strict, and in the direction RFC 0037
    /// requires beyond it:
    ///
    /// - unknown keys, unknown `kind` tokens, unknown fragments, and unknown
    ///   comparison operators are rejected, never ignored;
    /// - a `normal_form` declaration is verified by re-normalizing, and a false
    ///   declaration is rejected;
    /// - the AST is normalized on the way in, so a claim decoded from an authoring
    ///   form and the same claim decoded from its normal form are one value with one
    ///   identity.
    ///
    /// # Errors
    ///
    /// [`PropertyDecodeError`], naming the field or the token that failed.
    pub fn decode(bytes: &[u8]) -> Result<Self, PropertyDecodeError> {
        let json = Json::parse(bytes)?;
        Self::from_json(&json)
    }

    /// Decode a claim from an already-parsed JSON value.
    ///
    /// # Errors
    ///
    /// [`PropertyDecodeError`], as [`Claim::decode`].
    pub fn from_json(json: &Json) -> Result<Self, PropertyDecodeError> {
        let fields = object(json, "claims[]")?;
        known_keys(
            fields,
            "claims[]",
            &["expression", "id", "kind", "observer"],
        )?;
        let unit = UnitKey::new(string_field(fields, "claims[].id")?)?;
        let kind_token = string_field(fields, "claims[].kind")?;
        let class =
            ClaimKind::from_wire(kind_token).ok_or_else(|| PropertyDecodeError::UnknownToken {
                field: "claims[].kind",
                token: kind_token.to_owned(),
            })?;
        let expression = PropertyExpression::from_json(fields.get("expression").ok_or(
            PropertyDecodeError::MissingField {
                field: "claims[].expression",
            },
        )?)?;
        let observer = match fields.get("observer") {
            // Absent and `null` are one meaning with one spelling; see the module
            // documentation.
            None | Some(Json::Null) => None,
            Some(Json::String(id)) => Some(id.clone()),
            Some(other) => {
                return Err(PropertyDecodeError::TypeMismatch {
                    field: "claims[].observer",
                    expected: "string or null",
                    found: other.type_name(),
                });
            }
        };
        Ok(Self::new(unit, class, expression, observer))
    }
}

impl PartialEq for Claim {
    fn eq(&self, other: &Self) -> bool {
        self.unit == other.unit
            && self.class == other.class
            && self.expression == other.expression
            && self.observer == other.observer
    }
}

impl Eq for Claim {}

/// Which of the two spellings a claim is being rendered into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    /// The ID2-reduced identity preimage.
    Identity,
    /// The schema-conformant contract document form.
    Artifact,
}

fn claim_json(
    unit: &UnitKey,
    class: ClaimKind,
    expression: &PropertyExpression,
    observer: Option<&str>,
    layer: Layer,
) -> Json {
    let expression_json = match layer {
        Layer::Identity => expression.identity_preimage_json(),
        Layer::Artifact => expression.artifact_json(),
    };
    Json::object([
        ("expression".to_owned(), expression_json),
        ("id".to_owned(), Json::String(unit.as_str().to_owned())),
        ("kind".to_owned(), Json::String(class.wire().to_owned())),
        (
            "observer".to_owned(),
            observer.map_or(Json::Null, |id| Json::String(id.to_owned())),
        ),
    ])
    .unwrap_or(Json::Null)
}

/// The `claims` array: a set of claims keyed by their unit keys.
///
/// # Why a keyed set and not a list
///
/// W1 requires `claims[].id` to be unique, and RFC 0031 keys the `properties`
/// classification by exactly that id. A `claims` array is therefore a *set* with a
/// declared key, and this type is that set: [`ClaimSet::from_claims`] rejects a
/// duplicate rather than keeping both, and the encoding emits claims in unit-key
/// order.
///
/// The consequence is deliberate: reordering the authored array does not change the
/// identity. ID5 sorts object *keys* and says nothing about array order, and ID7's
/// checkable list says "reformatting it […] or reordering keys MUST NOT change
/// [the identity]" while "changing any claim […] MUST". Reordering a keyed set is a
/// reformatting, not a change to any claim, so admitting authored order into the
/// preimage would move an identity with every field classified `unchanged` — the
/// failure mode ID3 records as remarkable when a *policy* edit causes it, and which
/// nothing sanctions for a cosmetic one. *Raised as a flag against RFC 0037: ID5
/// should state the ordering rule for keyed arrays; every group with a declared unit
/// key (`assumptions`, `observers`, `nondeterminism`, `abstraction_maps`,
/// `fairness`) has the same question, and they must all answer it the same way.*
#[derive(Debug, Clone)]
pub struct ClaimSet {
    claims: BTreeMap<UnitKey, Claim>,
    identity: PropertyIdentity,
}

impl ClaimSet {
    /// Build a claim set, enforcing W1.
    ///
    /// # Errors
    ///
    /// - [`PropertyError::DuplicateUnitKey`] — W1: "`claims[].id` […] MUST each be
    ///   unique within their array […] duplicates make the diff's unit keying
    ///   ambiguous, which is a soundness hazard rather than a matter of taste."
    /// - [`PropertyError::NoClaims`] — the schema's `minItems: 1`: "Every
    ///   array-valued group MAY be empty except `claims`, which MUST carry at least
    ///   one claim."
    pub fn from_claims(claims: impl IntoIterator<Item = Claim>) -> Result<Self, PropertyError> {
        let mut keyed: BTreeMap<UnitKey, Claim> = BTreeMap::new();
        for claim in claims {
            if let Some(existing) = keyed.insert(claim.unit().clone(), claim) {
                return Err(PropertyError::DuplicateUnitKey {
                    unit: existing.unit().to_string(),
                });
            }
        }
        if keyed.is_empty() {
            return Err(PropertyError::NoClaims);
        }
        let identity = PropertyIdentity::of_bytes(
            Json::Array(
                keyed
                    .values()
                    .map(|claim| {
                        claim_json(
                            claim.unit(),
                            claim.class(),
                            claim.expression(),
                            claim.observer(),
                            Layer::Identity,
                        )
                    })
                    .collect(),
            )
            .to_canonical_bytes(),
        );
        Ok(Self {
            claims: keyed,
            identity,
        })
    }

    /// The `claims` portion of the contract's identity preimage.
    #[must_use]
    pub const fn identity(&self) -> &PropertyIdentity {
        &self.identity
    }

    /// How many claims the set carries. Always at least one.
    #[must_use]
    pub fn len(&self) -> usize {
        self.claims.len()
    }

    /// Always `false`; a claim set carries at least one claim.
    ///
    /// Present because clippy asks for it beside [`len`](Self::len), and it is a fair
    /// question to be able to ask of a collection even when the answer is fixed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.claims.is_empty()
    }

    /// The claim under `unit`, if any.
    #[must_use]
    pub fn get(&self, unit: &UnitKey) -> Option<&Claim> {
        self.claims.get(unit)
    }

    /// Every claim, in unit-key order.
    pub fn iter(&self) -> impl Iterator<Item = &Claim> {
        self.claims.values()
    }

    /// The artifact-form JSON array.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        Json::Array(self.claims.values().map(Claim::artifact_json).collect())
    }

    /// The artifact-form bytes.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// Decode a claim set from its artifact form.
    ///
    /// # Errors
    ///
    /// [`PropertyDecodeError`], including [`PropertyDecodeError::Property`] for the
    /// W1 and `minItems` violations the array shape carries.
    pub fn decode(bytes: &[u8]) -> Result<Self, PropertyDecodeError> {
        let json = Json::parse(bytes)?;
        Self::from_json(&json)
    }

    /// Decode a claim set from an already-parsed JSON array.
    ///
    /// # Errors
    ///
    /// [`PropertyDecodeError`], as [`ClaimSet::decode`].
    pub fn from_json(json: &Json) -> Result<Self, PropertyDecodeError> {
        let items = json
            .as_array()
            .ok_or_else(|| PropertyDecodeError::TypeMismatch {
                field: "claims",
                expected: "array",
                found: json.type_name(),
            })?;
        let claims = items
            .iter()
            .map(Claim::from_json)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::from_claims(claims)?)
    }

    /// Every observer this set binds, for the W2 check.
    ///
    /// W2: "A non-null `claims[].observer` MUST name an existing `observers[].id`. A
    /// dangling reference is rejected; it MUST NOT be treated as an unbound claim."
    /// The `observers` group is a sibling PR-4 bone, so this returns the references
    /// and [`ClaimSet::check_observers`] takes the declared ids.
    #[must_use]
    pub fn observer_references(&self) -> BTreeSet<&str> {
        self.claims.values().filter_map(Claim::observer).collect()
    }

    /// W2, given the declared `observers[].id` set.
    ///
    /// # Errors
    ///
    /// [`WellFormednessError::DanglingObserver`] for the first reference, in unit-key
    /// order, that names no declared observer.
    pub fn check_observers(&self, declared: &BTreeSet<String>) -> Result<(), WellFormednessError> {
        for claim in self.claims.values() {
            if let Some(observer) = claim.observer()
                && !declared.contains(observer)
            {
                return Err(WellFormednessError::DanglingObserver {
                    unit: claim.unit().to_string(),
                    observer: observer.to_owned(),
                });
            }
        }
        Ok(())
    }

    /// W3's expression half, given the contract's declared `scope.fragments`.
    ///
    /// > **W3 — Fragments are declared.** `scope.fragments` MUST be non-empty. An
    /// > expression's `fragment`, when present, MUST be a member of
    /// > `scope.fragments`. When `scope.fragments` names exactly one fragment, an
    /// > absent `expression.fragment` denotes that fragment; when it names more than
    /// > one, `expression.fragment` MUST be present, because "the intent's
    /// > `scope.fragments` apply" has no single reading otherwise.
    /// >
    /// > — RFC 0037
    ///
    /// # Errors
    ///
    /// [`WellFormednessError::NoDeclaredFragments`],
    /// [`WellFormednessError::UndeclaredFragment`], or
    /// [`WellFormednessError::AmbiguousFragment`].
    pub fn check_fragments(
        &self,
        declared: &BTreeSet<Fragment>,
    ) -> Result<(), WellFormednessError> {
        if declared.is_empty() {
            return Err(WellFormednessError::NoDeclaredFragments);
        }
        for claim in self.claims.values() {
            match claim.expression().fragment() {
                Some(fragment) if !declared.contains(&fragment) => {
                    return Err(WellFormednessError::UndeclaredFragment {
                        unit: claim.unit().to_string(),
                        fragment,
                    });
                }
                Some(_) => {}
                None if declared.len() > 1 => {
                    return Err(WellFormednessError::AmbiguousFragment {
                        unit: claim.unit().to_string(),
                        declared: declared.len(),
                    });
                }
                None => {}
            }
        }
        Ok(())
    }
}

impl PartialEq for ClaimSet {
    fn eq(&self, other: &Self) -> bool {
        self.claims == other.claims
    }
}

impl Eq for ClaimSet {}

impl<'a> IntoIterator for &'a ClaimSet {
    type Item = &'a Claim;
    type IntoIter = std::collections::btree_map::Values<'a, UnitKey, Claim>;

    fn into_iter(self) -> Self::IntoIter {
        self.claims.values()
    }
}

// --- errors --------------------------------------------------------------------------------

/// Why a property could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyError {
    /// A claim id is the empty string.
    EmptyUnitKey,
    /// Two claims share a `claims[].id` (W1).
    DuplicateUnitKey {
        /// The repeated key.
        unit: String,
    },
    /// The claim set is empty, which the schema's `minItems: 1` forbids.
    NoClaims,
    /// The expression's AST could not be normalized.
    Normalize(NormalizeError),
}

impl fmt::Display for PropertyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyUnitKey => {
                f.write_str("a claim id is its classified-unit key (RFC 0031) and cannot be empty")
            }
            Self::DuplicateUnitKey { unit } => write!(
                f,
                "two claims share the id {unit:?}; W1 requires unique unit keys because the \
                 intent diff keys classification by them"
            ),
            Self::NoClaims => f.write_str(
                "an intent contract carries at least one claim (intent-contract.schema.json, \
                 claims.minItems)",
            ),
            Self::Normalize(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for PropertyError {}

impl From<NormalizeError> for PropertyError {
    fn from(error: NormalizeError) -> Self {
        Self::Normalize(error)
    }
}

/// Why a byte string is not a property artifact.
///
/// Every variant is a rejection, never a repair. RFC 0037: "an intent contract is
/// never best-effort decoded".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyDecodeError {
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
    UnknownField {
        /// The object's path.
        field: &'static str,
        /// The offending key.
        key: String,
    },
    /// A closed vocabulary carries a token this build does not implement.
    ///
    /// The fail-closed case: a `next` node, a `cpnf-2` declaration, a seventh claim
    /// kind. "Forward compatibility is achieved by rejecting, never by ignoring."
    UnknownToken {
        /// The field's path.
        field: &'static str,
        /// The unrecognized token.
        token: String,
    },
    /// An expression declared `normal_form: "cpnf-1"` and its AST is not in CPNF-1.
    FalseNormalForm,
    /// The decoded AST is not a shape the schema admits.
    Ast(AstError),
    /// The decoded AST could not be normalized.
    Normalize(NormalizeError),
    /// The decoded claims do not form a well-formed set.
    Property(PropertyError),
}

impl fmt::Display for PropertyDecodeError {
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
                 the schema rejects"
            ),
            Self::UnknownToken { field, token } => write!(
                f,
                "`{field}` carries the token {token:?}, which this build does not implement; an \
                 unrecognized token in a closed vocabulary is rejected, never ignored (RFC 0037)"
            ),
            Self::FalseNormalForm => f.write_str(
                "the expression declares normal_form \"cpnf-1\" and its AST is not in CPNF-1; a \
                 declaration is verified by re-normalizing (RFC 0037 N9)",
            ),
            Self::Ast(error) => error.fmt(f),
            Self::Normalize(error) => error.fmt(f),
            Self::Property(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for PropertyDecodeError {}

impl From<JsonError> for PropertyDecodeError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

impl From<AstError> for PropertyDecodeError {
    fn from(error: AstError) -> Self {
        Self::Ast(error)
    }
}

impl From<NormalizeError> for PropertyDecodeError {
    fn from(error: NormalizeError) -> Self {
        Self::Normalize(error)
    }
}

impl From<PropertyError> for PropertyDecodeError {
    fn from(error: PropertyError) -> Self {
        match error {
            PropertyError::Normalize(error) => Self::Normalize(error),
            other => Self::Property(other),
        }
    }
}

/// Why a claim set is not well formed against the rest of a contract.
///
/// Separate from [`PropertyDecodeError`] because these rules are *relational*: they
/// hold between the `claims` group and a sibling group, so they cannot be decided
/// when a claim is decoded and are not the decoder's to report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WellFormednessError {
    /// W2: a `claims[].observer` names no declared observer.
    DanglingObserver {
        /// The claim's unit key.
        unit: String,
        /// The unresolved reference.
        observer: String,
    },
    /// W3: `scope.fragments` is empty.
    NoDeclaredFragments,
    /// W3: an expression declares a fragment `scope.fragments` does not.
    UndeclaredFragment {
        /// The claim's unit key.
        unit: String,
        /// The undeclared fragment.
        fragment: Fragment,
    },
    /// W3: an expression omits its fragment where more than one is declared.
    AmbiguousFragment {
        /// The claim's unit key.
        unit: String,
        /// How many fragments the contract declares.
        declared: usize,
    },
}

impl fmt::Display for WellFormednessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DanglingObserver { unit, observer } => write!(
                f,
                "claim {unit:?} binds the observer {observer:?}, which no observers[].id \
                 declares; a dangling reference is rejected, never read as an unbound claim (W2)"
            ),
            Self::NoDeclaredFragments => f.write_str(
                "scope.fragments is empty; every unit would classify unsupported and every \
                 promotion would be blocked, silently (W3)",
            ),
            Self::UndeclaredFragment { unit, fragment } => write!(
                f,
                "claim {unit:?} is stated in the {fragment} fragment, which scope.fragments does \
                 not declare (W3)"
            ),
            Self::AmbiguousFragment { unit, declared } => write!(
                f,
                "claim {unit:?} declares no fragment and the contract declares {declared}; \
                 \"the intent's scope.fragments apply\" has no single reading (W3)"
            ),
        }
    }
}

impl core::error::Error for WellFormednessError {}

#[cfg(test)]
mod tests {
    use super::*;
    use continuum_value::identity::{ContentHasher, Digest256, Fnv1aPlaceholder, HashAlgorithm};

    use crate::ast::{Binder, ComparisonOperator, Identifier, Literal, Term};

    fn ident(name: &str) -> Identifier {
        Identifier::new(name).expect("a test identifier is well formed")
    }

    fn unit(id: &str) -> UnitKey {
        UnitKey::new(id).expect("a test unit key is non-empty")
    }

    fn state(name: &str) -> Term {
        Term::State {
            name: ident(name),
            indices: Vec::new(),
        }
    }

    fn integer(value: i64) -> Term {
        Term::Literal {
            value: Literal::Integer(value),
        }
    }

    fn ack_implies_durable() -> Formula {
        Formula::always(Formula::forall(
            Binder::new(
                ident("e"),
                Term::Constant {
                    name: ident("Events"),
                },
            ),
            Formula::implies(
                Formula::predicate(ident("acknowledged"), vec![Term::Var { name: ident("e") }]),
                Formula::predicate(ident("durable"), vec![Term::Var { name: ident("e") }]),
            ),
        ))
    }

    fn expression(ast: &Formula) -> PropertyExpression {
        PropertyExpression::normalized(ast, Some(Fragment::Finite), None).expect("normalizes")
    }

    fn claim(id: &str, ast: &Formula) -> Claim {
        Claim::new(unit(id), ClaimKind::Safety, expression(ast), None)
    }

    fn utf8(bytes: &[u8]) -> String {
        String::from_utf8(bytes.to_vec()).expect("canonical output is UTF-8")
    }

    // --- the four parts of a property --------------------------------------------

    #[test]
    fn a_claim_carries_its_unit_key_class_normalized_formula_and_identity() {
        let claim = Claim::new(
            unit("C-ack-durable"),
            ClaimKind::Safety,
            PropertyExpression::normalized(
                &ack_implies_durable(),
                Some(Fragment::Finite),
                Some("always (forall e in Events: acknowledged[e] => durable[e])".to_owned()),
            )
            .expect("normalizes"),
            Some("O-ledger".to_owned()),
        );
        assert_eq!(claim.unit().as_str(), "C-ack-durable");
        assert_eq!(claim.class(), ClaimKind::Safety);
        assert_eq!(claim.expression().fragment(), Some(Fragment::Finite));
        assert!(claim.expression().source().is_some());
        assert_eq!(claim.observer(), Some("O-ledger"));
        // The stored AST is the normal form, not the authoring form: `implies` is
        // gone and the binder has been renamed.
        let ast = utf8(&crate::cpnf::encode(claim.expression().ast()));
        assert!(!ast.contains("implies"), "{ast}");
        assert!(ast.contains(r#""variable":"v0""#), "{ast}");
        // The identity is the preimage bytes themselves, readable as JSON.
        assert_eq!(
            claim.identity().canonical_bytes(),
            claim.identity().to_string().as_bytes()
        );
    }

    #[test]
    fn a_unit_key_cannot_be_empty() {
        assert_eq!(UnitKey::new(""), Err(PropertyError::EmptyUnitKey));
        assert!(UnitKey::new("C-1").is_ok());
    }

    #[test]
    fn the_claim_kind_vocabulary_is_closed() {
        for kind in ClaimKind::ALL {
            assert_eq!(ClaimKind::from_wire(kind.wire()), Some(kind));
        }
        assert_eq!(ClaimKind::ALL.len(), 6);
        assert_eq!(ClaimKind::from_wire("Safety"), None);
        assert_eq!(ClaimKind::from_wire("availability"), None);
    }

    // --- the two spellings, and the ID2 exclusion list ----------------------------

    #[test]
    fn id2_excludes_source_and_normal_form_from_the_identity_and_nothing_else() {
        let base = Claim::new(
            unit("C-1"),
            ClaimKind::Safety,
            PropertyExpression::normalized(&ack_implies_durable(), Some(Fragment::Finite), None)
                .expect("normalizes"),
            None,
        );
        let with_source = Claim::new(
            unit("C-1"),
            ClaimKind::Safety,
            PropertyExpression::normalized(
                &ack_implies_durable(),
                Some(Fragment::Finite),
                Some("a completely different rendering".to_owned()),
            )
            .expect("normalizes"),
            None,
        );
        // "rewriting `source` […] MUST NOT change [the identity]" (ID7).
        assert_eq!(base.identity(), with_source.identity());
        // …and the artifact form does carry it, so the document is not lossy.
        assert_ne!(base.to_artifact_bytes(), with_source.to_artifact_bytes());
        assert!(utf8(&with_source.to_artifact_bytes()).contains("completely different"));
        // `normal_form` is declared in the artifact form and absent from the preimage.
        assert!(utf8(&base.to_artifact_bytes()).contains(r#""normal_form":"cpnf-1""#));
        assert!(!utf8(base.identity().canonical_bytes()).contains("normal_form"));
        assert!(!utf8(base.identity().canonical_bytes()).contains("source"));
    }

    #[test]
    fn id7_every_semantic_part_moves_the_identity() {
        let base = claim("C-1", &ack_implies_durable());
        let weakened = Formula::always(Formula::forall(
            Binder::new(
                ident("e"),
                Term::Constant {
                    name: ident("Events"),
                },
            ),
            Formula::implies(
                Formula::predicate(ident("acknowledged"), vec![Term::Var { name: ident("e") }]),
                Formula::predicate(
                    ident("eventually_durable"),
                    vec![Term::Var { name: ident("e") }],
                ),
            ),
        ));
        let variants = [
            // a different unit key is a different claim (RFC 0031: a rename is
            // `removed` plus `added`, never `unchanged`)
            Claim::new(
                unit("C-2"),
                ClaimKind::Safety,
                expression(&ack_implies_durable()),
                None,
            ),
            // a different class, same expression (correction 7: `incomparable`)
            Claim::new(
                unit("C-1"),
                ClaimKind::Liveness,
                expression(&ack_implies_durable()),
                None,
            ),
            // a different observer binding
            Claim::new(
                unit("C-1"),
                ClaimKind::Safety,
                expression(&ack_implies_durable()),
                Some("O-ledger".to_owned()),
            ),
            // a different declared fragment
            Claim::new(
                unit("C-1"),
                ClaimKind::Safety,
                PropertyExpression::normalized(
                    &ack_implies_durable(),
                    Some(Fragment::Symbolic),
                    None,
                )
                .expect("normalizes"),
                None,
            ),
            // no declared fragment at all — a distinct meaning under W3
            Claim::new(
                unit("C-1"),
                ClaimKind::Safety,
                PropertyExpression::normalized(&ack_implies_durable(), None, None)
                    .expect("normalizes"),
                None,
            ),
            // a weakened property
            claim("C-1", &weakened),
        ];
        for (index, variant) in variants.iter().enumerate() {
            assert_ne!(
                base.identity(),
                variant.identity(),
                "variant {index} shares the base identity"
            );
            assert_ne!(&base, variant, "variant {index} compares equal to the base");
        }
    }

    #[test]
    fn a_rewritten_property_keeps_its_identity() {
        // The other half of ID7: the CPNF-1 rewrites are not changes.
        let authored = claim("C-1", &ack_implies_durable());
        let rewritten = claim(
            "C-1",
            &Formula::not(Formula::eventually(Formula::exists(
                Binder::new(
                    ident("x"),
                    Term::Constant {
                        name: ident("Events"),
                    },
                ),
                Formula::and(vec![
                    Formula::predicate(ident("acknowledged"), vec![Term::Var { name: ident("x") }]),
                    Formula::not(Formula::predicate(
                        ident("durable"),
                        vec![Term::Var { name: ident("x") }],
                    )),
                ])
                .expect("two operands"),
            ))),
        );
        assert_eq!(authored.identity(), rewritten.identity());
        assert_eq!(authored, rewritten);
    }

    #[test]
    fn an_absent_observer_and_an_explicit_null_are_one_spelling() {
        let claim = claim("C-1", &ack_implies_durable());
        let artifact = utf8(&claim.to_artifact_bytes());
        assert!(artifact.contains(r#""observer":null"#), "{artifact}");
        let without = artifact.replace(r#""observer":null,"#, "");
        let decoded = Claim::decode(without.as_bytes()).expect("decodes");
        assert_eq!(decoded.identity(), claim.identity());
        assert_eq!(decoded.to_artifact_bytes(), claim.to_artifact_bytes());
    }

    #[test]
    fn an_absent_fragment_is_a_distinct_meaning_and_is_not_invented() {
        let with = claim("C-1", &ack_implies_durable());
        let without = Claim::new(
            unit("C-1"),
            ClaimKind::Safety,
            PropertyExpression::normalized(&ack_implies_durable(), None, None).expect("normalizes"),
            None,
        );
        assert!(!utf8(&without.to_artifact_bytes()).contains("fragment"));
        assert_ne!(with.identity(), without.identity());
        assert_eq!(
            Claim::decode(&without.to_artifact_bytes())
                .expect("decodes")
                .expression()
                .fragment(),
            None
        );
    }

    // --- round-trip ---------------------------------------------------------------

    #[test]
    fn a_claim_round_trips_through_its_artifact_form() {
        let claim = Claim::new(
            unit("C-ack-durable"),
            ClaimKind::Safety,
            PropertyExpression::normalized(
                &ack_implies_durable(),
                Some(Fragment::Finite),
                Some("always (forall e in Events: acknowledged[e] => durable[e])".to_owned()),
            )
            .expect("normalizes"),
            Some("O-ledger".to_owned()),
        );
        let bytes = claim.to_artifact_bytes();
        let decoded = Claim::decode(&bytes).expect("decodes");
        assert_eq!(decoded, claim);
        assert_eq!(decoded.to_artifact_bytes(), bytes);
        assert_eq!(decoded.identity(), claim.identity());
        assert_eq!(decoded.expression().source(), claim.expression().source());
    }

    #[test]
    fn decoding_an_authoring_form_yields_the_same_value_as_its_normal_form() {
        // The same claim written the long way, with reordered object keys and
        // whitespace: JSON's own slack, normalized away.
        let authored = br#"{
            "kind" : "safety",
            "id"   : "C-1",
            "expression": {
                "fragment": "Finite",
                "ast": {
                    "kind": "always",
                    "operand": {
                        "kind": "forall",
                        "binder": {"variable": "e", "domain": {"kind":"constant","name":"Events"}},
                        "body": {
                            "kind": "implies",
                            "antecedent": {"kind":"predicate","name":"acknowledged","args":[{"kind":"var","name":"e"}]},
                            "consequent": {"kind":"predicate","name":"durable","args":[{"kind":"var","name":"e"}]}
                        }
                    }
                }
            }
        }"#;
        let decoded = Claim::decode(authored).expect("decodes");
        assert_eq!(decoded, claim("C-1", &ack_implies_durable()));
        assert_eq!(
            decoded.identity(),
            claim("C-1", &ack_implies_durable()).identity()
        );
    }

    // --- the claim set ------------------------------------------------------------

    #[test]
    fn w1_rejects_a_duplicate_unit_key() {
        let error = ClaimSet::from_claims([
            claim("C-1", &ack_implies_durable()),
            claim(
                "C-1",
                &Formula::always(Formula::predicate(ident("p"), Vec::new())),
            ),
        ])
        .expect_err("W1 must reject a duplicate id");
        assert_eq!(
            error,
            PropertyError::DuplicateUnitKey {
                unit: "C-1".to_owned()
            }
        );
    }

    #[test]
    fn a_claim_set_carries_at_least_one_claim() {
        assert_eq!(ClaimSet::from_claims([]), Err(PropertyError::NoClaims));
    }

    #[test]
    fn authored_order_does_not_reach_the_identity() {
        let a = claim(
            "C-a",
            &Formula::always(Formula::predicate(ident("p"), Vec::new())),
        );
        let b = claim(
            "C-b",
            &Formula::always(Formula::predicate(ident("q"), Vec::new())),
        );
        let one = ClaimSet::from_claims([a.clone(), b.clone()]).expect("two distinct keys");
        let other = ClaimSet::from_claims([b, a]).expect("two distinct keys");
        assert_eq!(one.identity(), other.identity());
        assert_eq!(one.to_artifact_bytes(), other.to_artifact_bytes());
        assert_eq!(one, other);
        // …and the encoded order is unit-key order.
        let encoded = utf8(&one.to_artifact_bytes());
        assert!(
            encoded.find(r#""id":"C-a""#) < encoded.find(r#""id":"C-b""#),
            "{encoded}"
        );
    }

    #[test]
    fn a_claim_set_round_trips_and_iterates_in_unit_key_order() {
        let set = ClaimSet::from_claims([
            claim(
                "C-b",
                &Formula::always(Formula::predicate(ident("q"), Vec::new())),
            ),
            claim(
                "C-a",
                &Formula::always(Formula::predicate(ident("p"), Vec::new())),
            ),
        ])
        .expect("two distinct keys");
        assert_eq!(set.len(), 2);
        assert!(!set.is_empty());
        assert_eq!(
            set.iter().map(|c| c.unit().to_string()).collect::<Vec<_>>(),
            vec!["C-a".to_owned(), "C-b".to_owned()]
        );
        assert!(set.get(&unit("C-a")).is_some());
        assert!(set.get(&unit("C-z")).is_none());
        let bytes = set.to_artifact_bytes();
        let decoded = ClaimSet::decode(&bytes).expect("decodes");
        assert_eq!(decoded, set);
        assert_eq!(decoded.identity(), set.identity());
        assert_eq!(decoded.to_artifact_bytes(), bytes);
        assert_eq!((&set).into_iter().count(), 2);
    }

    // --- the relational well-formedness rules ------------------------------------

    #[test]
    fn w2_rejects_a_dangling_observer_reference() {
        let set = ClaimSet::from_claims([Claim::new(
            unit("C-1"),
            ClaimKind::Safety,
            expression(&ack_implies_durable()),
            Some("O-missing".to_owned()),
        )])
        .expect("one claim");
        assert_eq!(set.observer_references(), BTreeSet::from(["O-missing"]));
        assert_eq!(
            set.check_observers(&BTreeSet::from(["O-ledger".to_owned()])),
            Err(WellFormednessError::DanglingObserver {
                unit: "C-1".to_owned(),
                observer: "O-missing".to_owned(),
            })
        );
        assert!(
            set.check_observers(&BTreeSet::from(["O-missing".to_owned()]))
                .is_ok()
        );
        // An unbound claim has no reference to dangle.
        let unbound =
            ClaimSet::from_claims([claim("C-1", &ack_implies_durable())]).expect("one claim");
        assert!(unbound.observer_references().is_empty());
        assert!(unbound.check_observers(&BTreeSet::new()).is_ok());
    }

    #[test]
    fn w3_checks_the_expression_half_of_the_fragment_rule() {
        let declared_one = BTreeSet::from([Fragment::Finite]);
        let declared_two = BTreeSet::from([Fragment::Finite, Fragment::Symbolic]);

        let with_fragment =
            ClaimSet::from_claims([claim("C-1", &ack_implies_durable())]).expect("one claim");
        assert!(with_fragment.check_fragments(&declared_one).is_ok());
        assert!(with_fragment.check_fragments(&declared_two).is_ok());
        assert_eq!(
            with_fragment.check_fragments(&BTreeSet::from([Fragment::Symbolic])),
            Err(WellFormednessError::UndeclaredFragment {
                unit: "C-1".to_owned(),
                fragment: Fragment::Finite,
            })
        );
        assert_eq!(
            with_fragment.check_fragments(&BTreeSet::new()),
            Err(WellFormednessError::NoDeclaredFragments)
        );

        let without_fragment = ClaimSet::from_claims([Claim::new(
            unit("C-1"),
            ClaimKind::Safety,
            PropertyExpression::normalized(&ack_implies_durable(), None, None).expect("normalizes"),
            None,
        )])
        .expect("one claim");
        // One declared fragment: absence denotes it.
        assert!(without_fragment.check_fragments(&declared_one).is_ok());
        // More than one: absence has no single reading.
        assert_eq!(
            without_fragment.check_fragments(&declared_two),
            Err(WellFormednessError::AmbiguousFragment {
                unit: "C-1".to_owned(),
                declared: 2,
            })
        );
    }

    // --- identity discipline ------------------------------------------------------

    /// A hasher that maps every input to one digest: the most hostile index a test
    /// can supply. Mirrors `continuum_value::identity`'s collision-injection suite.
    struct TotalCollision;

    impl ContentHasher for TotalCollision {
        const ALGORITHM: HashAlgorithm = HashAlgorithm::non_cryptographic("test-total-collision");

        fn hash(_bytes: &[u8]) -> Digest256 {
            Digest256::from_bytes([0u8; 32])
        }
    }

    #[test]
    fn identity_equality_cannot_consult_a_hash() {
        let a = claim("C-1", &ack_implies_durable());
        let b = claim(
            "C-1",
            &Formula::always(Formula::predicate(ident("p"), Vec::new())),
        );
        // Under a maximally colliding hash the digests agree…
        assert_eq!(
            a.identity().digest::<TotalCollision>(),
            b.identity().digest::<TotalCollision>()
        );
        // …and the identities do not, because an identity *is* the canonical bytes
        // and comparing them never reaches a hasher (ADR-0013).
        assert_ne!(a.identity(), b.identity());
        assert_ne!(a, b);
        // The placeholder hasher separates them, and that changes nothing about the
        // answer above: the certified relation is hash-independent.
        assert_ne!(
            a.identity().digest::<Fnv1aPlaceholder>(),
            b.identity().digest::<Fnv1aPlaceholder>()
        );
        assert_eq!(
            a.identity().digest::<Fnv1aPlaceholder>(),
            claim("C-1", &ack_implies_durable())
                .identity()
                .digest::<Fnv1aPlaceholder>()
        );
    }

    #[test]
    fn there_is_no_way_to_change_a_claim_in_place() {
        // Not a runtime assertion: the compile-time fact is that no `&mut self`
        // method exists. What is testable is the consequence — a "changed" claim is
        // a new value, and the original is untouched and keeps its identity.
        let original = claim("C-1", &ack_implies_durable());
        let before = original.identity().clone();
        let revised = Claim::new(
            original.unit().clone(),
            ClaimKind::Liveness,
            original.expression().clone(),
            None,
        );
        assert_eq!(original.identity(), &before);
        assert_ne!(revised.identity(), &before);
    }

    #[test]
    fn a_comparison_claim_encodes_with_its_operands_canonicalized() {
        // A small end-to-end shape check over the term sort, with N6 firing.
        let authored = Formula::always(Formula::compare(
            ComparisonOperator::Ge,
            state("big"),
            integer(0),
        ));
        let claim = claim("C-nonneg", &authored);
        let encoded = utf8(&claim.to_artifact_bytes());
        assert!(encoded.contains(r#""op":"le""#), "{encoded}");
        assert!(!encoded.contains(r#""op":"ge""#), "{encoded}");
        assert_eq!(
            claim.identity(),
            self::claim(
                "C-nonneg",
                &Formula::always(Formula::compare(
                    ComparisonOperator::Le,
                    integer(0),
                    state("big"),
                ))
            )
            .identity()
        );
    }
}
