//! Content identity per ADR-0013: exact canonical identity in certified lanes,
//! labeled 256-bit-hash identity nowhere else (PR 2).
//!
//! # What this module is for
//!
//! > Implement:
//! >
//! > - […]
//! > - content identity per ADR-0013: in certified lanes, canonical content
//! >   identity is primary and hash collisions are resolved by canonical
//! >   comparison; 256-bit-hash identity only in explicitly labeled
//! >   non-certified modes;
//! > - collision-injection tests; […]
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 2
//!
//! The decision itself is one paragraph, and every type below exists to make one
//! of its clauses unstateable in the wrong way:
//!
//! > Canonical structural encodings define identity. Hashes index and partition;
//! > collisions resolve by exact comparison. Artifact digests use cryptographic
//! > hashes, but proof validity still derives from decoded structure.
//! >
//! > — `notes/plan/adr/0013-exact-state-identity.md`, "Decision"
//!
//! and its second rejected alternative fixes what the escape hatch costs:
//!
//! > 2. 256-bit hashes as identity: extremely safe but still an assumption;
//! >    acceptable only for non-certified modes if labeled.
//! >
//! > — ADR-0013, "Alternatives considered"
//!
//! ## The three clauses, as three types
//!
//! - **"Canonical structural encodings define identity."** [`ContentIdentity`]
//!   *is* the [`Value::encode`] byte string — not a digest of it, not a struct
//!   containing a digest. Its [`PartialEq`] is byte equality on canonical
//!   encodings, so `ContentIdentity::of(a) == ContentIdentity::of(b)` holds
//!   exactly when `a == b`. There is no type parameter, no hasher argument, and
//!   no field a hash could reach: the certified lane's identity relation is
//!   *structurally independent* of which hash function the workspace eventually
//!   vendors. That independence is the point of the whole module, and
//!   `certified_identity_equality_cannot_consult_a_hash` pins it against the
//!   most hostile hash a test can write.
//! - **"Hashes index and partition; collisions resolve by exact comparison."**
//!   [`IdentityIndex`] is that sentence, executable: a
//!   `BTreeMap<Digest256, Vec<ContentIdentity>>` whose lookups locate a bucket by
//!   digest and then decide membership by comparing canonical bytes. A collision
//!   is neither an error nor a silent suppression — it is a bucket with more than
//!   one identity in it, reported by [`Insertion::Fresh::hash_collisions`] and
//!   enumerable through [`IdentityIndex::collisions`].
//! - **"acceptable only for non-certified modes if labeled."** [`HashIdentity`]
//!   compares digests, and it cannot be built without a [`NonCertifiedLabel`] —
//!   the constructor takes one by value, the label type has no `Default` and no
//!   infallible constructor, and there is no conversion from [`ContentIdentity`]
//!   into it. [`Identity`] carries the split as a two-variant enum, so a
//!   consumer that receives an identity has already been told which guarantee it
//!   holds; [`Identity::Certified`] and [`Identity::NonCertified`] are never
//!   equal, even for the same value.
//!
//! ## Why the mode split is a type and not a flag
//!
//! Plan §20 states the rule as a dependency-level obligation, not a runtime
//! option:
//!
//! > in certified lanes, content identity is exact canonical identity; hash
//! > collisions are resolved by canonical comparison; 256-bit-hash identity is
//! > permitted only in explicitly labeled non-certified modes (ADR-0013).
//! >
//! > — `notes/plan/plan.md` §20, "Dependency rules"
//!
//! A boolean `certified: bool` threaded through a call chain has a default, and a
//! default is exactly how an unlabeled lane happens. Here the weaker mode has no
//! spelling that omits its label: [`HashIdentity::compute`] requires a
//! [`NonCertifiedLabel`], [`NonCertifiedLabel::new`] returns a `Result` and
//! rejects the empty string, and [`HashIdentity`]'s [`Display`](fmt::Display)
//! prints `<label>/<algorithm>:<digest>` so the weaker guarantee survives into
//! any log line, receipt field, or error message that renders it. INV-008's
//! rule — never a bare boolean, never a success flag that outruns the evidence —
//! is the same rule one level down.
//!
//! Downgrading is likewise absent rather than convenient. There is no
//! `From<ContentIdentity> for HashIdentity` and no `ContentIdentity::into_hash`:
//! replacing an exact identity with a probabilistic one weakens a guarantee, and
//! under INV-011 that is a privileged revision with a semantic diff, not a method
//! call. A caller who wants a hash identity computes one from the value, in a
//! lane that has already named itself.
//!
//! # The hash seam, and the placeholder standing in it
//!
//! ADR-0013 says artifact digests use *cryptographic* hashes. This workspace has
//! zero external dependencies, and the decision about which hash to vendor — and
//! the review that decision needs — is not this module's to make. Hand-rolling a
//! cryptographic primitive to fill the gap would be worse than the gap: it would
//! produce something that *looks* like a release hash and is not.
//!
//! So the hash is a seam. [`ContentHasher`] is the whole contract — a
//! [`HashAlgorithm`] label and a pure `&[u8] -> Digest256` function — and
//! [`Fnv1aPlaceholder`] is a deliberately, visibly inadequate implementation of
//! it:
//!
//! - its algorithm token is `placeholder-fnv1a-256`, so any artifact, receipt, or
//!   log line that records the algorithm records the word "placeholder";
//! - [`HashAlgorithm::is_cryptographic`] returns `false` for it, so the weakness
//!   is machine-readable and not only prose;
//! - its documentation states outright that four FNV-1a lanes over one input are
//!   strongly correlated and that the construction has nothing resembling
//!   128-bit collision resistance, let alone 256.
//!
//! Replacing it is a one-line change at every call site (the turbofish on
//! [`ContentIdentity::digest`], [`HashIdentity::compute`], and
//! [`IdentityIndex`]), and it changes **no** certified identity: the tests that
//! state ADR-0013's semantics are parameterized over the hasher and are run
//! against a maximally colliding one. That is the property that makes deferring
//! the vendor decision safe rather than merely convenient.
//!
//! # How this reaches the store
//!
//! > Content-addressed identities are derivable by anyone holding the content —
//! > the §4.5 existence-oracle rule exists because of this — so handles are
//! > identifiers, not secrets, and confer no authority.
//! >
//! > — `notes/plan/plan.md` §4.4, "Content-addressed artifacts"
//!
//! A store path needs a short fixed-width token, and
//! `crates/continuum-workspace/src/artifact_path.rs` restricts one to
//! `[A-Za-z0-9_-]`. [`Digest256::to_token`] produces 64 lowercase hex characters,
//! which is inside that class — so a digest is spellable as an
//! `ArtifactHandle` identity without any escaping, and
//! `digest_tokens_are_artifact_path_identity_characters` pins that (the two
//! crates share no dependency edge; `continuum-value` is a leaf, so the coupling
//! is a checked assertion about a character class, not an import).
//!
//! That token is an *index*, not the identity. ADR-0013's last clause —
//! "proof validity still derives from decoded structure" — is why
//! [`ContentIdentity::from_canonical_bytes`] exists and why it decodes: a store
//! that fetches by digest must re-derive the identity from the bytes it got back
//! and compare canonically, exactly as [`IdentityIndex`] does in memory. A fetch
//! that trusts the digest it asked for has re-introduced the assumption ADR-0013
//! removed.
//!
//! # Determinism obligations
//!
//! > […] different hash seeds where internal structures allow […] Semantic
//! > artifacts must be identical or differences must be explicitly non-semantic
//! > and normalized away.
//! >
//! > — `notes/plan/docs/19_TEST_STRATEGY.md` §7, "Determinism matrix"
//!
//! §7's hash-seed dimension is met here in its strongest form:
//! [`IdentityIndex::sorted_identities`] returns the same listing under *different
//! hash functions*, not merely different seeds, and
//! `a_listing_is_the_same_under_every_hash` asserts it across the placeholder and
//! two adversarial test hashes. Nothing in this module reads a clock, the
//! environment, an allocator address, or a `std` hasher; every container is a
//! [`BTreeMap`] and every bucket is kept in canonical order, so iteration order
//! is a function of content alone.
//!
//! No type here implements [`Hash`](core::hash::Hash), for the reason
//! [`Value`] does not (see `value.rs`): a `Hash` impl is the affordance that lets
//! a caller index by fingerprint and never compare canonically, and
//! `HashMap`-ordered iteration over identities would fail §7's matrix. Use a
//! [`BTreeMap`] keyed by [`ContentIdentity`], or [`IdentityIndex`], which does
//! the hashing where the comparison is guaranteed to follow.
//!
//! # Why this module is in `continuum-value`
//!
//! Plan §20 lists `continuum-value` and its dependency rules state ADR-0013 as
//! one of them, so identity belongs to the crate the rule is written about.
//! Structurally there is nowhere else it could go: an identity *is*
//! [`Value::encode`] bytes, so any other home would either import
//! `continuum-value` (adding an edge for a type that is already this crate's
//! vocabulary) or duplicate the encoding (two spellings of one identity — the
//! failure ADR-0013 exists to prevent). `notes/plan/docs/33_REVISION_3_ARCHITECTURE.md`
//! puts *content identity* inside the smallest trust base alongside the canonical
//! value decoder, and this is the workspace's only declared leaf crate, so the
//! kernel, the certificate formats, the evidence graph and `continuumd` can all
//! agree on one identity vocabulary at a cost of zero new dependency edges.
//!
//! # Seams left open on purpose
//!
//! - **Atomic publication and authorization are not here** (INV-017, plan §4.4,
//!   PR 2's remaining slices). [`IdentityIndex::insert`] is the in-memory core
//!   they need — idempotent, collision-resolving, and returning
//!   [`Insertion::Existing`] when canonical comparison finds the content already
//!   present, which is precisely "concurrent publication of identical artifacts
//!   yields one identity" with the concurrency and the durability removed. A
//!   store, a lock, a fsync order, and a capability check are not a leaf crate's
//!   business.
//! - **The release hash is not here** — see "The hash seam" above.
//! - **Artifact handles are not here.** A handle is a class prefix plus an
//!   identity token (plan §4.4); the class vocabulary and the path layout live in
//!   `continuum-workspace`. This module supplies the token and asserts it fits.
//! - **Signatures and receipts are not here.** A digest is not an attestation;
//!   who vouched for an artifact is RFC 0024's question.

use core::fmt;
use core::marker::PhantomData;
use std::collections::BTreeMap;
use std::collections::btree_map::Iter as BTreeMapIter;

use crate::value::{DecodeError, Value};

// --- digests ---------------------------------------------------------------------------

/// Width of a content digest in bits — ADR-0013's "256-bit hashes".
pub const DIGEST_BITS: usize = 256;

/// Width of a content digest in bytes.
pub const DIGEST_LEN: usize = DIGEST_BITS / 8;

/// How many characters a [`Digest256::to_token`] spelling has.
pub const DIGEST_TOKEN_LEN: usize = DIGEST_LEN * 2;

const HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";

/// A 256-bit digest of a canonical encoding.
///
/// Opaque bytes with one canonical spelling. [`Ord`] is byte order, so a
/// [`BTreeMap`] keyed by a digest iterates in an order that depends on the digests
/// present and on nothing else (docs/19 §7).
///
/// A digest is never an identity on its own. In the certified lane it indexes
/// [`ContentIdentity`] values that are then compared canonically; in an explicitly
/// labeled non-certified lane it is wrapped in [`HashIdentity`], which carries the
/// label that says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Digest256([u8; DIGEST_LEN]);

impl Digest256 {
    /// Wrap 32 raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; DIGEST_LEN]) -> Self {
        Self(bytes)
    }

    /// The raw bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; DIGEST_LEN] {
        &self.0
    }

    /// The digest as 64 lowercase hex characters.
    ///
    /// Lowercase hex is inside `[A-Za-z0-9_-]`, the identity character class
    /// `crates/continuum-workspace/src/artifact_path.rs` accepts, so this token is
    /// directly usable as the identity half of a plan §4.4 artifact handle.
    #[must_use]
    pub fn to_token(&self) -> String {
        let mut token = String::with_capacity(DIGEST_TOKEN_LEN);
        for byte in self.0 {
            token.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
            token.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
        }
        token
    }

    /// Recover a digest from its [`to_token`](Self::to_token) spelling.
    ///
    /// Uppercase hex is rejected rather than accepted and lowercased: under
    /// ADR-0013 an identity has exactly one canonical spelling, and normalizing
    /// here would create a second one.
    ///
    /// # Errors
    ///
    /// [`IdentityError::DigestTokenCharacter`] for anything outside `[0-9a-f]` and
    /// [`IdentityError::DigestTokenLength`] when the token is not exactly
    /// [`DIGEST_TOKEN_LEN`] characters.
    pub fn from_token(token: &str) -> Result<Self, IdentityError> {
        if let Some((index, character)) = token
            .char_indices()
            .find(|&(_, c)| !matches!(c, '0'..='9' | 'a'..='f'))
        {
            return Err(IdentityError::DigestTokenCharacter { index, character });
        }
        // Every character is now ASCII, so byte length and character count agree.
        if token.len() != DIGEST_TOKEN_LEN {
            return Err(IdentityError::DigestTokenLength { len: token.len() });
        }
        let mut digest = [0u8; DIGEST_LEN];
        for (index, pair) in token.as_bytes().chunks_exact(2).enumerate() {
            digest[index] = (nibble(pair[0]) << 4) | nibble(pair[1]);
        }
        Ok(Self(digest))
    }
}

/// Decode one validated lowercase hex digit.
///
/// The caller has already rejected every byte outside `[0-9a-f]`, so the final arm
/// is unreachable; it returns `0` rather than panicking because a trust-base
/// component does not abort on its own invariant.
const fn nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => 0,
    }
}

impl fmt::Display for Digest256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_token())
    }
}

// --- the hash seam ---------------------------------------------------------------------

/// The identity and honesty of a hash function.
///
/// Two fields, both load-bearing. `token` is what an artifact records so that
/// "which hash" is never a guess; `cryptographic` is the machine-readable half of
/// ADR-0013's distinction between "artifact digests use cryptographic hashes" and
/// a stand-in that does not.
///
/// A token is expected to match `[a-z0-9-]+` so it can appear unescaped in a
/// handle, a path, or a receipt field; [`HashAlgorithm::is_well_formed`] checks it
/// and `every_algorithm_token_is_well_formed` asserts it for every hasher this
/// crate defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HashAlgorithm {
    token: &'static str,
    cryptographic: bool,
}

impl HashAlgorithm {
    /// Declare a hash the project has accepted as cryptographic.
    ///
    /// Reserved for a vendored, reviewed primitive. Nothing in this crate uses it
    /// yet — see the module documentation's "hash seam" section.
    #[must_use]
    pub const fn cryptographic(token: &'static str) -> Self {
        Self {
            token,
            cryptographic: true,
        }
    }

    /// Declare a hash that is *not* cryptographic: a placeholder, a test double,
    /// or a fast partitioner.
    ///
    /// An identity built on one of these is honest about it —
    /// [`HashIdentity::is_cryptographic`] reports `false` and the algorithm token
    /// travels with the identity.
    #[must_use]
    pub const fn non_cryptographic(token: &'static str) -> Self {
        Self {
            token,
            cryptographic: false,
        }
    }

    /// The stable machine name of this algorithm.
    #[must_use]
    pub const fn token(self) -> &'static str {
        self.token
    }

    /// Whether the project has accepted this hash as cryptographic.
    #[must_use]
    pub const fn is_cryptographic(self) -> bool {
        self.cryptographic
    }

    /// Whether the token is a non-empty `[a-z0-9-]+` word.
    #[must_use]
    pub fn is_well_formed(self) -> bool {
        !self.token.is_empty()
            && self
                .token
                .chars()
                .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-'))
    }
}

impl fmt::Display for HashAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token)
    }
}

/// A pure 256-bit hash over canonical encoding bytes.
///
/// The entire seam between ADR-0013's semantics and the hash-vendor decision. An
/// implementation must be a *function of its input alone* — no seed, no clock, no
/// interior state — because docs/19 §7 requires the artifacts derived from it to
/// be identical across processes, machines, and restarts.
///
/// Implementations are zero-sized types used only through the turbofish
/// (`identity.digest::<Fnv1aPlaceholder>()`), so swapping the workspace's hash is
/// a type substitution, never a data migration of anything certified: no
/// [`ContentIdentity`] mentions a hasher.
pub trait ContentHasher {
    /// This hasher's algorithm label, recorded wherever its digests are.
    const ALGORITHM: HashAlgorithm;

    /// Hash a canonical encoding.
    fn hash(bytes: &[u8]) -> Digest256;
}

/// FNV-1a offset basis, 64-bit.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a prime, 64-bit.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// **Not the release hash.** A non-cryptographic stand-in that fills the
/// [`ContentHasher`] seam until the dependency decision lands.
///
/// Four 64-bit FNV-1a lanes, domain-separated by a leading lane byte, concatenated
/// big-endian into 256 bits. FNV-1a is a well-known, deliberately simple,
/// deliberately non-cryptographic construction; running it four times over the
/// same input produces four strongly correlated lanes, so this hash has nothing
/// resembling 128-bit collision resistance, let alone the 256 bits its output
/// width suggests. Second preimages are trivial to construct. It is here because
/// something must be, and because being obviously inadequate is safer than being
/// plausibly inadequate.
///
/// What it *is* good for: deterministically partitioning identities into buckets so
/// that [`IdentityIndex`] does fewer canonical comparisons than a linear scan. That
/// is the only job ADR-0013 gives a hash in a certified lane, and this discharges
/// it correctly.
///
/// Replacing it changes every digest and every artifact *token*, and changes no
/// [`ContentIdentity`] at all. Certified identities are canonical encodings, so
/// they are not invalidated by a hash change; the digests pinned in this module's
/// tests are labeled as placeholder-specific and are expected to change with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fnv1aPlaceholder;

impl Fnv1aPlaceholder {
    /// One domain-separated FNV-1a lane.
    fn lane(index: u8, bytes: &[u8]) -> u64 {
        let mut state = FNV_OFFSET_BASIS ^ u64::from(index);
        state = state.wrapping_mul(FNV_PRIME);
        for byte in bytes {
            state ^= u64::from(*byte);
            state = state.wrapping_mul(FNV_PRIME);
        }
        state
    }
}

impl ContentHasher for Fnv1aPlaceholder {
    const ALGORITHM: HashAlgorithm = HashAlgorithm::non_cryptographic("placeholder-fnv1a-256");

    fn hash(bytes: &[u8]) -> Digest256 {
        let mut digest = [0u8; DIGEST_LEN];
        for lane in 0..4u8 {
            let value = Self::lane(lane, bytes).to_be_bytes();
            let at = usize::from(lane) * 8;
            digest[at..at + 8].copy_from_slice(&value);
        }
        Digest256::from_bytes(digest)
    }
}

// --- certified identity ----------------------------------------------------------------

/// The identity of a value in a certified lane: its canonical CVNF-1 encoding.
///
/// Not a digest of the encoding — the encoding. ADR-0013's first sentence is
/// "canonical structural encodings define identity", and this type is that
/// sentence with no room left between the definition and the representation:
/// [`PartialEq`] compares canonical bytes, so two identities are equal exactly
/// when their values are, unconditionally and for every hash function that has
/// ever been or will ever be plugged into [`ContentHasher`].
///
/// [`Ord`] is byte order on those encodings, which `value.rs` proves is the same
/// relation as [`Value`]'s own [`Ord`] (`value_order_is_lexicographic_order_on_encodings`).
/// A [`BTreeMap`] keyed by [`ContentIdentity`] therefore iterates in value order.
///
/// There is deliberately no [`Hash`](core::hash::Hash) impl; see the module
/// documentation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContentIdentity {
    canonical: Box<[u8]>,
}

impl ContentIdentity {
    /// The identity of `value`.
    ///
    /// Infallible: every [`Value`] has a canonical encoding.
    #[must_use]
    pub fn of(value: &Value) -> Self {
        Self {
            canonical: value.encode().into_boxed_slice(),
        }
    }

    /// Recover an identity from bytes that claim to be a canonical encoding.
    ///
    /// This is the verification step a content-addressed fetch owes ADR-0013:
    /// "proof validity still derives from decoded structure". The bytes are
    /// *decoded*, which rejects every non-canonical spelling — a leading zero in
    /// an integer magnitude, an out-of-order set element, a zero multiplicity,
    /// trailing bytes — so an accepted byte string is the unique encoding of the
    /// value it denotes, and the identity is stored as the re-encoding of that
    /// value rather than as the caller's bytes.
    ///
    /// # Errors
    ///
    /// The [`DecodeError`] naming the first violation and its byte offset.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        Value::decode(bytes).map(|value| Self::of(&value))
    }

    /// The canonical encoding: this identity, as bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }

    /// The value this identity denotes.
    ///
    /// ADR-0013 keeps proof validity on the decoded structure, not on the address,
    /// so a checker holding an identity can always get the value back rather than
    /// reasoning about a digest.
    ///
    /// # Errors
    ///
    /// The [`DecodeError`] from the canonical decoder. Unreachable for an identity
    /// built through this module's constructors — both of them decode or encode a
    /// real [`Value`] — and reported rather than unwrapped because a trust-base
    /// component does not abort on its own invariant.
    pub fn to_value(&self) -> Result<Value, DecodeError> {
        Value::decode(&self.canonical)
    }

    /// This identity's digest under `H`, for indexing only.
    ///
    /// The digest partitions; it never decides equality. Nothing in this type's
    /// [`PartialEq`], [`Ord`], or storage depends on `H`.
    #[must_use]
    pub fn digest<H: ContentHasher>(&self) -> Digest256 {
        H::hash(&self.canonical)
    }

    /// The canonical encoding as lowercase hex.
    ///
    /// Deliberately *not* abbreviated. The identity is the whole byte string, and
    /// a shortened rendering would be a digest wearing an identity's name. Where a
    /// fixed-width token is required — a store path, a handle — use
    /// [`digest`](Self::digest) and [`Digest256::to_token`], which are honestly an
    /// index.
    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut hex = String::with_capacity(self.canonical.len() * 2);
        for byte in &self.canonical {
            hex.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
            hex.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
        }
        hex
    }
}

impl fmt::Display for ContentIdentity {
    /// `cvnf-1:<hex>` — the encoding identifier and the identity itself.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", crate::value::ENCODING_ID, self.to_hex())
    }
}

// --- labeled non-certified identity ------------------------------------------------------

/// The name of a lane that has accepted 256-bit-hash identity.
///
/// ADR-0013 permits hash-only identity "for non-certified modes if labeled", and
/// this is the label. It is a required constructor argument rather than an
/// `Option<&str>` field, has no [`Default`], and cannot be built from the empty
/// string, so there is no spelling of [`HashIdentity::compute`] that omits it.
///
/// The character class is `[A-Za-z0-9_-]`, matching the artifact-handle identity
/// class of `crates/continuum-workspace/src/artifact_path.rs`, so a label can be
/// embedded in a token, a path, or a structured log field without escaping.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NonCertifiedLabel(String);

impl NonCertifiedLabel {
    /// Name a non-certified lane.
    ///
    /// # Errors
    ///
    /// [`IdentityError::EmptyLabel`] for the empty string — an unnamed lane is an
    /// unlabeled lane, which is the thing ADR-0013 forbids — and
    /// [`IdentityError::LabelCharacter`] for the first character outside
    /// `[A-Za-z0-9_-]`.
    pub fn new(label: &str) -> Result<Self, IdentityError> {
        if label.is_empty() {
            return Err(IdentityError::EmptyLabel);
        }
        if let Some((index, character)) = label
            .char_indices()
            .find(|&(_, c)| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        {
            return Err(IdentityError::LabelCharacter { index, character });
        }
        Ok(Self(label.to_owned()))
    }

    /// The label text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NonCertifiedLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A 256-bit-hash identity, usable only in the labeled lane it names.
///
/// Equality is digest equality *and* algorithm equality — two digests from
/// different hash functions are never the same identity — and it is therefore an
/// *assumption*, not a fact: distinct values with a colliding digest are equal
/// here. That is the whole content of ADR-0013's rejected alternative 2
/// ("extremely safe but still an assumption"), and this type is where the
/// assumption is allowed to live, in the open.
///
/// The label takes no part in equality: the same value hashed in two lanes is one
/// artifact under one digest, and making the lane name part of identity would
/// fragment a cache rather than protect anything. The label travels with the
/// identity for reporting — [`Display`](fmt::Display) always renders it — so the
/// weaker guarantee cannot be observed without also observing its lane.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HashIdentity {
    algorithm: HashAlgorithm,
    digest: Digest256,
    label: NonCertifiedLabel,
}

impl HashIdentity {
    /// Compute the hash identity of `value` in the lane named by `label`.
    ///
    /// The label is taken by value and has no default. This signature *is* the
    /// enforcement of "acceptable only for non-certified modes if labeled": the
    /// unlabeled call does not compile.
    #[must_use]
    pub fn compute<H: ContentHasher>(value: &Value, label: NonCertifiedLabel) -> Self {
        Self {
            algorithm: H::ALGORITHM,
            digest: H::hash(&value.encode()),
            label,
        }
    }

    /// The digest this identity is.
    #[must_use]
    pub const fn digest(&self) -> Digest256 {
        self.digest
    }

    /// Which hash produced it.
    #[must_use]
    pub const fn algorithm(&self) -> HashAlgorithm {
        self.algorithm
    }

    /// The lane that accepted hash-only identity.
    #[must_use]
    pub const fn label(&self) -> &NonCertifiedLabel {
        &self.label
    }

    /// Whether the hash behind this identity is one the project accepts as
    /// cryptographic.
    ///
    /// `false` for [`Fnv1aPlaceholder`], and for every test double. A lane that
    /// checks this before trusting a digest gets a second, machine-readable
    /// warning on top of the label.
    #[must_use]
    pub const fn is_cryptographic(&self) -> bool {
        self.algorithm.is_cryptographic()
    }
}

impl fmt::Display for HashIdentity {
    /// `<label>/<algorithm>:<digest token>`.
    ///
    /// The label comes first so that a rendered non-certified identity cannot be
    /// mistaken for a certified one at a glance, in a log line, or in a diff.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}:{}", self.label, self.algorithm, self.digest)
    }
}

// --- the mode split --------------------------------------------------------------------

/// Which of ADR-0013's two identity regimes an [`Identity`] belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IdentityMode {
    /// Exact canonical identity. Collisions are impossible, not improbable.
    CertifiedCanonical,
    /// 256-bit-hash identity in an explicitly labeled non-certified lane.
    /// Collision-freedom is an assumption.
    NonCertifiedHash256,
}

impl IdentityMode {
    /// Both modes, strongest first.
    pub const ALL: [Self; 2] = [Self::CertifiedCanonical, Self::NonCertifiedHash256];

    /// The stable machine name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CertifiedCanonical => "certified-canonical",
            Self::NonCertifiedHash256 => "non-certified-hash256",
        }
    }

    /// Whether this mode is usable in a certified lane.
    #[must_use]
    pub const fn is_certified(self) -> bool {
        matches!(self, Self::CertifiedCanonical)
    }
}

impl fmt::Display for IdentityMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An identity together with the guarantee it carries.
///
/// A consumer that accepts an [`Identity`] must match on it, so "which mode is
/// this" is answered at the type level before the identity can be used. The two
/// variants are never equal, even for the same value: an exact identity and a
/// probabilistic one are different claims, and letting them compare equal would
/// let a non-certified result satisfy a certified lookup — the exact silent
/// downgrade ADR-0013 and INV-011 exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Identity {
    /// Exact canonical identity (ADR-0013, certified lanes).
    Certified(ContentIdentity),
    /// Labeled 256-bit-hash identity (ADR-0013, non-certified modes only).
    NonCertified(HashIdentity),
}

impl Identity {
    /// The certified identity of `value`. Always available, needs no hash.
    #[must_use]
    pub fn certified(value: &Value) -> Self {
        Self::Certified(ContentIdentity::of(value))
    }

    /// The hash identity of `value` in the lane named by `label`.
    ///
    /// As with [`HashIdentity::compute`], the label is required.
    #[must_use]
    pub fn non_certified<H: ContentHasher>(value: &Value, label: NonCertifiedLabel) -> Self {
        Self::NonCertified(HashIdentity::compute::<H>(value, label))
    }

    /// Which regime this identity belongs to.
    #[must_use]
    pub const fn mode(&self) -> IdentityMode {
        match self {
            Self::Certified(_) => IdentityMode::CertifiedCanonical,
            Self::NonCertified(_) => IdentityMode::NonCertifiedHash256,
        }
    }

    /// Whether this identity is usable in a certified lane.
    #[must_use]
    pub const fn is_certified(&self) -> bool {
        self.mode().is_certified()
    }

    /// The canonical identity, if this is a certified one.
    #[must_use]
    pub const fn canonical(&self) -> Option<&ContentIdentity> {
        match self {
            Self::Certified(identity) => Some(identity),
            Self::NonCertified(_) => None,
        }
    }

    /// The hash identity, if this is a labeled non-certified one.
    #[must_use]
    pub const fn hash_only(&self) -> Option<&HashIdentity> {
        match self {
            Self::Certified(_) => None,
            Self::NonCertified(identity) => Some(identity),
        }
    }

    /// The non-certified lane label, if there is one.
    ///
    /// `None` for a certified identity — not because the label is missing, but
    /// because a certified identity has no lane to disclaim.
    #[must_use]
    pub const fn non_certified_label(&self) -> Option<&NonCertifiedLabel> {
        match self {
            Self::Certified(_) => None,
            Self::NonCertified(identity) => Some(identity.label()),
        }
    }
}

impl fmt::Display for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Certified(identity) => fmt::Display::fmt(identity, f),
            Self::NonCertified(identity) => fmt::Display::fmt(identity, f),
        }
    }
}

// --- hashes index and partition ----------------------------------------------------------

/// What an [`IdentityIndex::insert`] did.
///
/// Never a bare boolean: a fresh insertion that resolved a collision and a fresh
/// insertion that did not are different facts about the store, and INV-008's rule
/// against success flags that outrun the evidence applies to a publication result
/// as much as to a verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Insertion {
    /// The identity was not present and has been interned.
    Fresh {
        /// How many *other* distinct identities already shared this digest.
        ///
        /// Non-zero means a hash collision occurred and canonical comparison
        /// resolved it — the ADR-0013 behaviour, observed rather than assumed
        /// away. With a cryptographic hash this is expected to be zero forever;
        /// with the placeholder, or under attack, it is data.
        hash_collisions: usize,
    },
    /// Canonical comparison found an equal identity already interned.
    ///
    /// This is the in-memory half of PR 2's exit criterion "concurrent publication
    /// of identical artifacts yields one identity"; the concurrency, the durability
    /// and the authorization belong to the atomic-publication slice.
    Existing,
}

impl Insertion {
    /// Whether this insertion added a new identity.
    #[must_use]
    pub const fn is_fresh(self) -> bool {
        matches!(self, Self::Fresh { .. })
    }

    /// How many collisions canonical comparison had to resolve.
    #[must_use]
    pub const fn hash_collisions(self) -> usize {
        match self {
            Self::Fresh { hash_collisions } => hash_collisions,
            Self::Existing => 0,
        }
    }
}

/// A set of certified identities, partitioned by digest.
///
/// > Hashes index and partition; collisions resolve by exact comparison.
/// >
/// > — ADR-0013
///
/// Both halves are here and neither is optional. The digest selects a bucket;
/// membership within the bucket is decided by comparing canonical encodings. A
/// weak hash — or an adversarial one — costs this structure *time*, in the form of
/// longer buckets, and can cost it nothing else: no lookup, no insertion, and no
/// listing consults a digest for anything but bucket selection.
///
/// Buckets are kept in canonical order, so iteration is a function of the
/// identities present and of the hash function, and
/// [`sorted_identities`](Self::sorted_identities) is a function of the identities
/// alone — identical under every hasher (docs/19 §7).
///
/// This is not a store. It has no atomicity, no durability, and no authorization;
/// those are PR 2's remaining slices (INV-017, plan §4.4).
#[derive(Debug, Clone)]
pub struct IdentityIndex<H: ContentHasher> {
    buckets: BTreeMap<Digest256, Vec<ContentIdentity>>,
    len: usize,
    hasher: PhantomData<fn() -> H>,
}

impl<H: ContentHasher> IdentityIndex<H> {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: BTreeMap::new(),
            len: 0,
            hasher: PhantomData,
        }
    }

    /// The algorithm partitioning this index.
    #[must_use]
    pub const fn algorithm() -> HashAlgorithm {
        H::ALGORITHM
    }

    /// Intern `identity`, resolving any digest collision by canonical comparison.
    pub fn insert(&mut self, identity: ContentIdentity) -> Insertion {
        let digest = identity.digest::<H>();
        let bucket = self.buckets.entry(digest).or_default();
        match bucket.binary_search(&identity) {
            Ok(_) => Insertion::Existing,
            Err(at) => {
                let hash_collisions = bucket.len();
                bucket.insert(at, identity);
                self.len += 1;
                Insertion::Fresh { hash_collisions }
            }
        }
    }

    /// The interned identity equal to `identity`, if any.
    ///
    /// Locates the bucket by digest, then compares canonically. A value whose
    /// digest collides with an interned one but whose encoding differs is *not*
    /// found — which is the whole difference between this and a fingerprint set.
    #[must_use]
    pub fn get(&self, identity: &ContentIdentity) -> Option<&ContentIdentity> {
        let bucket = self.bucket(identity.digest::<H>());
        bucket
            .binary_search(identity)
            .ok()
            .and_then(|at| bucket.get(at))
    }

    /// Whether an equal identity is interned.
    #[must_use]
    pub fn contains(&self, identity: &ContentIdentity) -> bool {
        self.get(identity).is_some()
    }

    /// Every identity sharing `digest`, in canonical order.
    #[must_use]
    pub fn bucket(&self, digest: Digest256) -> &[ContentIdentity] {
        self.buckets.get(&digest).map_or(&[], Vec::as_slice)
    }

    /// How many distinct identities are interned.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the index is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// How many distinct digests the identities occupy.
    ///
    /// Below [`len`](Self::len) exactly when the hash has collided.
    #[must_use]
    pub fn digest_count(&self) -> usize {
        self.buckets.len()
    }

    /// Every bucket holding more than one identity: the collisions this index has
    /// absorbed, in digest order.
    ///
    /// A collision is reportable data. A certified lane keeps working through one;
    /// a lane that also cares about the *health* of its hash can watch this, and a
    /// non-empty result under a hash the project calls cryptographic is a defect
    /// report, not a warning.
    pub fn collisions(&self) -> impl Iterator<Item = (Digest256, &[ContentIdentity])> {
        self.buckets
            .iter()
            .filter(|(_, bucket)| bucket.len() > 1)
            .map(|(digest, bucket)| (*digest, bucket.as_slice()))
    }

    /// Every `(digest, bucket)` pair, in digest order.
    pub fn buckets(&self) -> BTreeMapIter<'_, Digest256, Vec<ContentIdentity>> {
        self.buckets.iter()
    }

    /// Every interned identity, in canonical order.
    ///
    /// Hash-independent: the same identities produce the same listing under every
    /// [`ContentHasher`], which is docs/19 §7's "different hash seeds" dimension
    /// discharged at its strongest. Use this for anything that gets published,
    /// diffed, or compared across processes; [`buckets`](Self::buckets) exposes the
    /// partition itself, which is hash-dependent by construction.
    #[must_use]
    pub fn sorted_identities(&self) -> Vec<&ContentIdentity> {
        let mut identities: Vec<&ContentIdentity> = self
            .buckets
            .values()
            .flat_map(|bucket| bucket.iter())
            .collect();
        identities.sort_unstable();
        identities
    }
}

impl<H: ContentHasher> Default for IdentityIndex<H> {
    fn default() -> Self {
        Self::new()
    }
}

// --- errors ----------------------------------------------------------------------------

/// Why a string is not a usable label or digest token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    /// A non-certified lane was named with the empty string.
    ///
    /// The typed way to have no label is to use a [`ContentIdentity`], which needs
    /// none.
    EmptyLabel,
    /// A lane label contained a character outside `[A-Za-z0-9_-]`.
    LabelCharacter {
        /// Byte offset of the first offending character.
        index: usize,
        /// The first offending character.
        character: char,
    },
    /// A digest token contained a character outside `[0-9a-f]`.
    ///
    /// Uppercase hex lands here: a digest has one spelling (ADR-0013).
    DigestTokenCharacter {
        /// Byte offset of the first offending character.
        index: usize,
        /// The first offending character.
        character: char,
    },
    /// A digest token was not [`DIGEST_TOKEN_LEN`] characters long.
    DigestTokenLength {
        /// The rejected length.
        len: usize,
    },
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLabel => f.write_str(
                "a non-certified identity lane needs a label (ADR-0013: hash-only identity \
                 is acceptable \"only for non-certified modes if labeled\")",
            ),
            Self::LabelCharacter { index, character } => write!(
                f,
                "lane label has a character {character:?} outside [A-Za-z0-9_-] at byte {index}"
            ),
            Self::DigestTokenCharacter { index, character } => write!(
                f,
                "digest token has a character {character:?} outside [0-9a-f] at byte {index}"
            ),
            Self::DigestTokenLength { len } => write!(
                f,
                "digest token is {len} characters, not {DIGEST_TOKEN_LEN}"
            ),
        }
    }
}

impl core::error::Error for IdentityError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{Name, ValueKind};

    // --- adversarial hashes -------------------------------------------------------------
    //
    // The collision-injection instruments. ADR-0013's certified-lane guarantee is not
    // "collisions are rare"; it is "collisions do not matter". The only way to test that
    // claim is to make them certain, so these hashers collide by construction and every
    // certified-lane property below is asserted while one of them is installed.

    /// Every input hashes to the same digest. The worst hash that exists.
    #[derive(Debug, Clone, Copy)]
    struct ConstantHash;

    impl ContentHasher for ConstantHash {
        const ALGORITHM: HashAlgorithm = HashAlgorithm::non_cryptographic("test-constant");

        fn hash(_bytes: &[u8]) -> Digest256 {
            Digest256::from_bytes([0x5a; DIGEST_LEN])
        }
    }

    /// Hashes only the kind tag, so every two values of the same kind collide.
    ///
    /// Nastier than [`ConstantHash`] in one specific way: it produces *several*
    /// non-trivial buckets, so a bug that happens to work when everything is in one
    /// bucket still fails here.
    #[derive(Debug, Clone, Copy)]
    struct KindTagHash;

    impl ContentHasher for KindTagHash {
        const ALGORITHM: HashAlgorithm = HashAlgorithm::non_cryptographic("test-kind-tag");

        fn hash(bytes: &[u8]) -> Digest256 {
            let mut digest = [0u8; DIGEST_LEN];
            digest[0] = bytes.first().copied().unwrap_or(0);
            Digest256::from_bytes(digest)
        }
    }

    /// Hashes the encoding length, so collisions follow a third, unrelated rule.
    #[derive(Debug, Clone, Copy)]
    struct LengthHash;

    impl ContentHasher for LengthHash {
        const ALGORITHM: HashAlgorithm = HashAlgorithm::non_cryptographic("test-length");

        fn hash(bytes: &[u8]) -> Digest256 {
            let mut digest = [0u8; DIGEST_LEN];
            digest[..8].copy_from_slice(&(bytes.len() as u64).to_be_bytes());
            Digest256::from_bytes(digest)
        }
    }

    // --- fixtures -----------------------------------------------------------------------

    fn name(text: &str) -> Name {
        Name::new(text).expect("test name is canonical")
    }

    fn symbol(text: &str) -> Value {
        Value::symbol(name(text))
    }

    /// A spread of values covering every kind, with several same-kind pairs so that
    /// [`KindTagHash`] produces populated buckets.
    fn sample_values() -> Vec<Value> {
        vec![
            Value::Null,
            Value::Bool(false),
            Value::Bool(true),
            Value::nat(0),
            Value::nat(1),
            Value::nat(u128::from(u64::MAX)),
            Value::int(-1),
            Value::int(0),
            Value::int(i128::MAX),
            Value::bitvec(8, 1).expect("valid bitvector"),
            Value::bitvec(16, 1).expect("valid bitvector"),
            Value::bytes(vec![]),
            Value::bytes(vec![0x00]),
            Value::bytes(vec![0xff, 0x00]),
            Value::text(""),
            Value::text("a"),
            Value::text("aa"),
            Value::text("z"),
            symbol("alpha"),
            symbol("beta"),
            Value::tuple([Value::Null, Value::nat(1)]).expect("within depth"),
            Value::tuple([Value::nat(1), Value::Null]).expect("within depth"),
            Value::seq([Value::nat(1), Value::nat(2)]).expect("within depth"),
            Value::seq([Value::nat(2), Value::nat(1)]).expect("within depth"),
            Value::set([Value::nat(1), Value::nat(2)]).expect("within depth"),
            Value::set([Value::nat(3)]).expect("within depth"),
            Value::multiset([(Value::nat(1), 2)]).expect("within depth"),
            Value::multiset([(Value::nat(1), 3)]).expect("within depth"),
            Value::record([(name("a"), Value::nat(1))]).expect("within depth"),
            Value::record([(name("a"), Value::nat(2))]).expect("within depth"),
            Value::variant(name("none"), Value::Null).expect("within depth"),
            Value::variant(name("some"), Value::nat(7)).expect("within depth"),
            Value::map([(Value::nat(1), Value::text("x"))]).expect("within depth"),
            Value::map([(Value::nat(1), Value::text("y"))]).expect("within depth"),
            Value::opaque(name("q"), vec![1, 2, 3]),
            Value::opaque(name("q"), vec![1, 2, 4]),
        ]
    }

    fn sample_identities() -> Vec<ContentIdentity> {
        sample_values().iter().map(ContentIdentity::of).collect()
    }

    fn label(text: &str) -> NonCertifiedLabel {
        NonCertifiedLabel::new(text).expect("test label is well formed")
    }

    // --- ADR-0013: the encoding is the identity ------------------------------------------

    #[test]
    fn a_certified_identity_is_the_canonical_encoding() {
        for value in sample_values() {
            let identity = ContentIdentity::of(&value);
            assert_eq!(identity.canonical_bytes(), value.encode().as_slice());
            assert_eq!(identity.to_value(), Ok(value));
        }
    }

    #[test]
    fn identities_are_equal_exactly_when_values_are() {
        let values = sample_values();
        for left in &values {
            for right in &values {
                assert_eq!(
                    ContentIdentity::of(left) == ContentIdentity::of(right),
                    left == right,
                    "{left:?} vs {right:?}"
                );
                // And the order agrees with the value order (value.rs pins that the
                // value order *is* byte order on these encodings).
                assert_eq!(
                    ContentIdentity::of(left).cmp(&ContentIdentity::of(right)),
                    left.cmp(right),
                    "{left:?} vs {right:?}"
                );
            }
        }
    }

    #[test]
    fn identity_is_a_function_of_the_value_alone() {
        // Same set, built in two orders; same map, built in two orders. value.rs pins
        // this for the encoding, and identity inherits it — a re-derivation on another
        // machine must land on the same identity (plan §4.4).
        let ascending = Value::set([Value::nat(1), Value::nat(2), Value::nat(3)]).expect("valid");
        let descending = Value::set([Value::nat(3), Value::nat(2), Value::nat(1)]).expect("valid");
        assert_eq!(
            ContentIdentity::of(&ascending),
            ContentIdentity::of(&descending)
        );
        assert_eq!(
            ContentIdentity::of(&ascending).digest::<Fnv1aPlaceholder>(),
            ContentIdentity::of(&descending).digest::<Fnv1aPlaceholder>()
        );
    }

    #[test]
    fn an_identity_round_trips_through_its_canonical_bytes() {
        for identity in sample_identities() {
            let recovered = ContentIdentity::from_canonical_bytes(identity.canonical_bytes())
                .expect("a canonical encoding decodes");
            assert_eq!(recovered, identity);
            assert_eq!(recovered.canonical_bytes(), identity.canonical_bytes());
        }
    }

    #[test]
    fn non_canonical_bytes_are_not_an_identity() {
        // Each of these is a *different spelling* of a value that already has one, and
        // accepting any of them would give one identity two encodings.
        let mut trailing = Value::nat(1).encode();
        trailing.push(0x01);
        assert!(ContentIdentity::from_canonical_bytes(&trailing).is_err());

        // A set whose elements descend: strictly-ascending is the canonical form.
        let unsorted = [
            ValueKind::Set.tag(),
            0x01,
            0x02, // two elements
            ValueKind::Nat.tag(),
            0x01,
            0x02, // nat 2
            ValueKind::Nat.tag(),
            0x01,
            0x01, // nat 1
        ];
        assert!(ContentIdentity::from_canonical_bytes(&unsorted).is_err());

        // A nat with a leading zero byte in its magnitude.
        let non_minimal = [ValueKind::Nat.tag(), 0x02, 0x00, 0x01];
        assert!(ContentIdentity::from_canonical_bytes(&non_minimal).is_err());

        // Empty input is not the encoding of anything.
        assert!(ContentIdentity::from_canonical_bytes(&[]).is_err());
    }

    // --- collision injection: the certified lane -----------------------------------------

    #[test]
    fn certified_identity_equality_cannot_consult_a_hash() {
        // The structural argument, made executable: `ContentIdentity` has no hasher
        // parameter, so equality is the same relation no matter what is installed. The
        // assertions below hold *while* every digest in sight is identical.
        let left = Value::text("a");
        let right = Value::text("b");

        assert_eq!(
            ConstantHash::hash(&left.encode()),
            ConstantHash::hash(&right.encode()),
            "the injected collision must actually collide"
        );
        assert_ne!(ContentIdentity::of(&left), ContentIdentity::of(&right));
        assert_eq!(ContentIdentity::of(&left), ContentIdentity::of(&left));
    }

    #[test]
    fn an_index_under_a_maximally_colliding_hash_still_separates_every_value() {
        let identities = sample_identities();
        let mut index: IdentityIndex<ConstantHash> = IdentityIndex::new();
        for identity in identities.clone() {
            index.insert(identity);
        }

        // One bucket for everything — the collision is total.
        assert_eq!(index.digest_count(), 1);
        // And yet not one identity was suppressed.
        assert_eq!(index.len(), identities.len());
        for identity in &identities {
            assert!(index.contains(identity), "{identity} was suppressed");
            assert_eq!(index.get(identity), Some(identity));
        }

        // The collision is reported, not hidden.
        let collisions: Vec<(Digest256, &[ContentIdentity])> = index.collisions().collect();
        assert_eq!(collisions.len(), 1);
        assert_eq!(collisions[0].1.len(), identities.len());
    }

    #[test]
    fn a_collision_is_counted_rather_than_absorbed() {
        let mut index: IdentityIndex<ConstantHash> = IdentityIndex::new();
        assert_eq!(
            index.insert(ContentIdentity::of(&Value::text("a"))),
            Insertion::Fresh { hash_collisions: 0 }
        );
        assert_eq!(
            index.insert(ContentIdentity::of(&Value::text("b"))),
            Insertion::Fresh { hash_collisions: 1 }
        );
        assert_eq!(
            index.insert(ContentIdentity::of(&Value::text("c"))),
            Insertion::Fresh { hash_collisions: 2 }
        );
        assert_eq!(index.len(), 3);
        assert_eq!(index.digest_count(), 1);

        // Under a hash that does not collide on this input, the same three values report
        // no collisions at all — so the counter reflects the hash, not the index.
        let mut honest: IdentityIndex<Fnv1aPlaceholder> = IdentityIndex::new();
        for text in ["a", "b", "c"] {
            assert_eq!(
                honest.insert(ContentIdentity::of(&Value::text(text))),
                Insertion::Fresh { hash_collisions: 0 }
            );
        }
        assert_eq!(honest.digest_count(), 3);
    }

    #[test]
    fn equal_values_get_one_identity_even_when_everything_collides() {
        let mut index: IdentityIndex<ConstantHash> = IdentityIndex::new();
        let value = Value::record([(name("k"), Value::nat(9))]).expect("valid");

        assert_eq!(
            index.insert(ContentIdentity::of(&value)),
            Insertion::Fresh { hash_collisions: 0 }
        );
        // The same value, reached by a different route: encoded, shipped, decoded.
        let shipped = ContentIdentity::from_canonical_bytes(&value.encode()).expect("canonical");
        assert_eq!(index.insert(shipped), Insertion::Existing);
        // And a value built independently.
        let rebuilt = Value::record([(name("k"), Value::nat(9))]).expect("valid");
        assert_eq!(
            index.insert(ContentIdentity::of(&rebuilt)),
            Insertion::Existing
        );

        assert_eq!(index.len(), 1);

        // A *different* record with the same colliding digest is not the same artifact.
        let other = Value::record([(name("k"), Value::nat(10))]).expect("valid");
        assert_eq!(
            index.insert(ContentIdentity::of(&other)),
            Insertion::Fresh { hash_collisions: 1 }
        );
        assert_eq!(index.len(), 2);
    }

    #[test]
    fn a_colliding_lookup_does_not_report_a_false_hit() {
        // The failure mode a fingerprint set has and this does not: asking for a value
        // that is absent but whose digest is present.
        let mut index: IdentityIndex<ConstantHash> = IdentityIndex::new();
        index.insert(ContentIdentity::of(&Value::text("present")));

        let absent = ContentIdentity::of(&Value::text("absent"));
        assert_eq!(
            absent.digest::<ConstantHash>(),
            ContentIdentity::of(&Value::text("present")).digest::<ConstantHash>()
        );
        assert!(!index.contains(&absent));
        assert_eq!(index.get(&absent), None);
        // The bucket is non-empty; membership still says no.
        assert_eq!(index.bucket(absent.digest::<ConstantHash>()).len(), 1);
    }

    #[test]
    fn every_injected_hash_gives_the_same_certified_answers() {
        // Three unrelated collision rules — everything collides, same-kind collides,
        // same-length collides — and one honest hash. The certified-lane facts are
        // identical in all four.
        fn check<H: ContentHasher>() {
            let identities = sample_identities();
            let mut index: IdentityIndex<H> = IdentityIndex::new();
            for identity in identities.clone() {
                index.insert(identity.clone());
                // Idempotent under every hash.
                assert_eq!(index.insert(identity), Insertion::Existing);
            }
            assert_eq!(index.len(), identities.len(), "{}", H::ALGORITHM);
            for identity in &identities {
                assert_eq!(index.get(identity), Some(identity), "{}", H::ALGORITHM);
            }
        }

        check::<ConstantHash>();
        check::<KindTagHash>();
        check::<LengthHash>();
        check::<Fnv1aPlaceholder>();
    }

    #[test]
    fn a_listing_is_the_same_under_every_hash() {
        // docs/19 §7's "different hash seeds where internal structures allow", in its
        // strongest form: different hash *functions*, identical semantic artifact.
        fn listing<H: ContentHasher>(order: &[ContentIdentity]) -> Vec<String> {
            let mut index: IdentityIndex<H> = IdentityIndex::new();
            for identity in order {
                index.insert(identity.clone());
            }
            index
                .sorted_identities()
                .into_iter()
                .map(ContentIdentity::to_hex)
                .collect()
        }

        let identities = sample_identities();
        let expected = listing::<Fnv1aPlaceholder>(&identities);
        assert_eq!(listing::<ConstantHash>(&identities), expected);
        assert_eq!(listing::<KindTagHash>(&identities), expected);
        assert_eq!(listing::<LengthHash>(&identities), expected);

        // …and independent of insertion order, too.
        let mut reversed = identities.clone();
        reversed.reverse();
        assert_eq!(listing::<KindTagHash>(&reversed), expected);
        let mut rotated = identities;
        rotated.rotate_left(7);
        assert_eq!(listing::<ConstantHash>(&rotated), expected);
    }

    #[test]
    fn buckets_stay_in_canonical_order() {
        let mut index: IdentityIndex<KindTagHash> = IdentityIndex::new();
        let mut identities = sample_identities();
        identities.reverse();
        for identity in identities {
            index.insert(identity);
        }
        for (_, bucket) in index.buckets() {
            assert!(
                bucket.windows(2).all(|pair| pair[0] < pair[1]),
                "bucket is unordered or holds a duplicate"
            );
        }
    }

    // --- the labeled non-certified mode ---------------------------------------------------

    #[test]
    fn a_non_certified_identity_conflates_exactly_what_its_hash_conflates() {
        // This is the weaker guarantee, demonstrated rather than described: under a
        // colliding hash, two distinct values *are* one non-certified identity. The
        // point of the mode split is that this can only happen where it is labeled.
        let left = Value::text("a");
        let right = Value::text("b");

        let left_id = HashIdentity::compute::<ConstantHash>(&left, label("replay-cache"));
        let right_id = HashIdentity::compute::<ConstantHash>(&right, label("replay-cache"));
        assert_eq!(left_id, right_id);

        // The certified lane, over the same two values, says what it always says.
        assert_ne!(ContentIdentity::of(&left), ContentIdentity::of(&right));
    }

    #[test]
    fn a_non_certified_identity_always_carries_its_label_and_algorithm() {
        let identity = HashIdentity::compute::<Fnv1aPlaceholder>(
            &Value::text("a"),
            label("bulk-dedup-preview"),
        );
        assert_eq!(identity.label().as_str(), "bulk-dedup-preview");
        assert_eq!(identity.algorithm().token(), "placeholder-fnv1a-256");
        assert!(!identity.is_cryptographic());

        // Rendering it discloses both, so the weaker guarantee survives into any log
        // line or diff that shows the identity.
        let rendered = identity.to_string();
        assert!(
            rendered.starts_with("bulk-dedup-preview/placeholder-fnv1a-256:"),
            "{rendered}"
        );
        assert!(
            rendered.ends_with(&identity.digest().to_token()),
            "{rendered}"
        );

        let wrapped = Identity::NonCertified(identity);
        assert_eq!(wrapped.mode(), IdentityMode::NonCertifiedHash256);
        assert!(!wrapped.is_certified());
        assert!(wrapped.canonical().is_none());
        assert_eq!(
            wrapped.non_certified_label().map(NonCertifiedLabel::as_str),
            Some("bulk-dedup-preview")
        );
    }

    #[test]
    fn a_non_certified_lane_cannot_be_entered_unlabeled() {
        // `HashIdentity::compute` takes a `NonCertifiedLabel` by value: there is no
        // default, no `Option`, and no `From<&str>`, so an unlabeled call does not
        // compile. What *can* be attempted at runtime is a vacuous label, and those are
        // rejected rather than accepted-and-ignored.
        assert_eq!(NonCertifiedLabel::new(""), Err(IdentityError::EmptyLabel));
        for bad in [
            " ",
            "  ",
            "\t",
            "lane name",
            "lane/name",
            "lane.name",
            "läne",
        ] {
            assert!(
                matches!(
                    NonCertifiedLabel::new(bad),
                    Err(IdentityError::LabelCharacter { .. })
                ),
                "label {bad:?} was accepted"
            );
        }
        assert!(NonCertifiedLabel::new("replay-cache_2").is_ok());
    }

    #[test]
    fn the_two_modes_are_never_equal() {
        let value = Value::text("a");
        let certified = Identity::certified(&value);
        let non_certified =
            Identity::non_certified::<Fnv1aPlaceholder>(&value, label("replay-cache"));

        assert_ne!(certified, non_certified);
        assert!(certified.is_certified());
        assert!(!non_certified.is_certified());
        assert_eq!(certified.mode(), IdentityMode::CertifiedCanonical);
        assert!(certified.non_certified_label().is_none());
        assert!(certified.hash_only().is_none());
        assert_eq!(certified.canonical(), Some(&ContentIdentity::of(&value)));

        // Two lanes, one artifact: the label does not fragment identity.
        let other_lane =
            Identity::non_certified::<Fnv1aPlaceholder>(&value, label("bulk-dedup-preview"));
        assert_eq!(
            non_certified.hash_only().map(HashIdentity::digest),
            other_lane.hash_only().map(HashIdentity::digest)
        );
    }

    #[test]
    fn identities_from_different_hashes_are_different_identities() {
        // Two hashes that agree on nothing except output width. An identity carries its
        // algorithm, so a digest computed under one is never silently accepted as one
        // computed under another.
        let value = Value::text("a");
        let placeholder = HashIdentity::compute::<Fnv1aPlaceholder>(&value, label("lane"));
        let constant = HashIdentity::compute::<ConstantHash>(&value, label("lane"));
        assert_ne!(placeholder, constant);
        assert_ne!(placeholder.algorithm(), constant.algorithm());
    }

    #[test]
    fn modes_have_stable_machine_names() {
        assert_eq!(
            IdentityMode::ALL.map(IdentityMode::as_str),
            ["certified-canonical", "non-certified-hash256"]
        );
        assert!(IdentityMode::CertifiedCanonical.is_certified());
        assert!(!IdentityMode::NonCertifiedHash256.is_certified());
        assert_eq!(
            IdentityMode::NonCertifiedHash256.to_string(),
            "non-certified-hash256"
        );
    }

    // --- the hash seam --------------------------------------------------------------------

    #[test]
    fn the_placeholder_declares_itself_non_cryptographic() {
        // The seam's honesty contract. If a real hash lands and this test is updated to
        // expect `true`, that edit is the visible record of the vendor decision.
        assert!(!Fnv1aPlaceholder::ALGORITHM.is_cryptographic());
        assert_eq!(Fnv1aPlaceholder::ALGORITHM.token(), "placeholder-fnv1a-256");
        assert!(
            Fnv1aPlaceholder::ALGORITHM.token().contains("placeholder"),
            "the algorithm token is the in-band label; it must say what it is"
        );
        assert_eq!(
            IdentityIndex::<Fnv1aPlaceholder>::algorithm(),
            Fnv1aPlaceholder::ALGORITHM
        );
    }

    #[test]
    fn every_algorithm_token_is_well_formed() {
        for algorithm in [
            Fnv1aPlaceholder::ALGORITHM,
            ConstantHash::ALGORITHM,
            KindTagHash::ALGORITHM,
            LengthHash::ALGORITHM,
            HashAlgorithm::cryptographic("some-vendored-hash-256"),
        ] {
            assert!(algorithm.is_well_formed(), "{algorithm}");
            assert_eq!(algorithm.to_string(), algorithm.token());
        }
        assert!(!HashAlgorithm::non_cryptographic("").is_well_formed());
        assert!(!HashAlgorithm::non_cryptographic("Has-Caps").is_well_formed());
        assert!(!HashAlgorithm::non_cryptographic("has space").is_well_formed());
    }

    #[test]
    fn the_placeholder_is_a_pure_function_of_the_bytes() {
        for value in sample_values() {
            let identity = ContentIdentity::of(&value);
            assert_eq!(
                identity.digest::<Fnv1aPlaceholder>(),
                identity.digest::<Fnv1aPlaceholder>()
            );
            assert_eq!(
                identity.digest::<Fnv1aPlaceholder>(),
                Fnv1aPlaceholder::hash(&value.encode())
            );
        }
    }

    #[test]
    fn the_placeholder_separates_the_sample_domain() {
        // Not a security claim — a usefulness claim. A partitioner that put the whole
        // sample domain in one bucket would still be *correct* under ADR-0013 and would
        // be worthless, so this checks the stand-in earns its place.
        let mut index: IdentityIndex<Fnv1aPlaceholder> = IdentityIndex::new();
        for identity in sample_identities() {
            index.insert(identity);
        }
        assert_eq!(index.digest_count(), index.len());
        assert_eq!(index.collisions().count(), 0);
    }

    // --- digest tokens ---------------------------------------------------------------------

    #[test]
    fn digest_tokens_are_artifact_path_identity_characters() {
        // `crates/continuum-workspace/src/artifact_path.rs` accepts `[A-Za-z0-9_-]` for
        // the identity half of a handle. `continuum-value` is a leaf crate and may not
        // import it, so the coupling is asserted here rather than typed.
        for identity in sample_identities() {
            let token = identity.digest::<Fnv1aPlaceholder>().to_token();
            assert_eq!(token.len(), DIGEST_TOKEN_LEN);
            assert!(
                token
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
                "{token}"
            );
            assert!(token.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
        }
    }

    #[test]
    fn digest_tokens_round_trip() {
        for identity in sample_identities() {
            let digest = identity.digest::<Fnv1aPlaceholder>();
            let token = digest.to_token();
            assert_eq!(Digest256::from_token(&token), Ok(digest));
            assert_eq!(digest.to_string(), token);
        }
        let zero = Digest256::from_bytes([0; DIGEST_LEN]);
        assert_eq!(zero.to_token(), "0".repeat(DIGEST_TOKEN_LEN));
        assert_eq!(Digest256::from_token(&zero.to_token()), Ok(zero));
        let ones = Digest256::from_bytes([0xff; DIGEST_LEN]);
        assert_eq!(ones.to_token(), "f".repeat(DIGEST_TOKEN_LEN));
        assert_eq!(ones.as_bytes(), &[0xff; DIGEST_LEN]);
    }

    #[test]
    fn malformed_digest_tokens_are_typed_errors() {
        assert_eq!(
            Digest256::from_token(""),
            Err(IdentityError::DigestTokenLength { len: 0 })
        );
        assert_eq!(
            Digest256::from_token(&"0".repeat(DIGEST_TOKEN_LEN - 1)),
            Err(IdentityError::DigestTokenLength {
                len: DIGEST_TOKEN_LEN - 1
            })
        );
        assert_eq!(
            Digest256::from_token(&"0".repeat(DIGEST_TOKEN_LEN + 1)),
            Err(IdentityError::DigestTokenLength {
                len: DIGEST_TOKEN_LEN + 1
            })
        );
        // Uppercase is a second spelling, so it is rejected, not normalized.
        assert_eq!(
            Digest256::from_token(&format!("A{}", "0".repeat(DIGEST_TOKEN_LEN - 1))),
            Err(IdentityError::DigestTokenCharacter {
                index: 0,
                character: 'A'
            })
        );
        assert_eq!(
            Digest256::from_token(&format!("{}g", "0".repeat(DIGEST_TOKEN_LEN - 1))),
            Err(IdentityError::DigestTokenCharacter {
                index: DIGEST_TOKEN_LEN - 1,
                character: 'g'
            })
        );
        // A non-ASCII character is reported by byte offset without slicing panics.
        assert!(matches!(
            Digest256::from_token("é0"),
            Err(IdentityError::DigestTokenCharacter {
                index: 0,
                character: 'é'
            })
        ));
    }

    // --- pinned vectors ----------------------------------------------------------------

    #[test]
    fn certified_identity_vectors_are_pinned() {
        // Literals, not recomputation. These are *identities*: changing one changes what
        // a published artifact is, which is a semantic-epoch event (plan §4.6), never a
        // fix. They are hash-independent by construction.
        let vectors: [(Value, &str); 5] = [
            (Value::Null, "cvnf-1:01"),
            (Value::Bool(true), "cvnf-1:0201"),
            (Value::nat(0), "cvnf-1:0300"),
            (Value::nat(258), "cvnf-1:03020102"),
            (Value::int(-1), "cvnf-1:0400fefe"),
        ];
        for (value, expected) in vectors {
            assert_eq!(ContentIdentity::of(&value).to_string(), expected);
        }
    }

    #[test]
    fn placeholder_digest_vectors_are_pinned() {
        // Pinned for *determinism*, not for stability across the hash-vendor decision:
        // these are `placeholder-fnv1a-256` outputs and they are expected to change when
        // a real hash lands. Nothing certified changes with them — see
        // `certified_identity_vectors_are_pinned`, which is the vector set that is an
        // epoch event to touch.
        let tokens: Vec<String> = [Value::Null, Value::Bool(true), Value::nat(0)]
            .iter()
            .map(|value| {
                ContentIdentity::of(value)
                    .digest::<Fnv1aPlaceholder>()
                    .to_token()
            })
            .collect();
        assert_eq!(
            tokens,
            [
                "08328707b4eb6e3a082f2307b4e88e7708395307b4f1348c0835ef07b4ee54c9",
                "d94645186c0967b2d0aa6418672cf911eaa6d31875e4f1ace1efc21870f169c3",
                "d942e0186c06863cd0adc918672fda87eaa36e1875e21036e1f3271870f44b39",
            ]
        );
    }

    // --- errors ---------------------------------------------------------------------------

    #[test]
    fn errors_render_their_cause() {
        assert!(IdentityError::EmptyLabel.to_string().contains("labeled"));
        assert!(
            IdentityError::LabelCharacter {
                index: 4,
                character: '/'
            }
            .to_string()
            .contains("byte 4")
        );
        assert!(
            IdentityError::DigestTokenCharacter {
                index: 0,
                character: 'Z'
            }
            .to_string()
            .contains("[0-9a-f]")
        );
        assert!(
            IdentityError::DigestTokenLength { len: 3 }
                .to_string()
                .contains("64")
        );
    }
}
