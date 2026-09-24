//! The declaration machinery: three macros that emit a Rust type *and* the
//! self-description the conformance test compares against the IDL.
//!
//! # Why the description is generated, not written beside the type
//!
//! The conformance test in `tests/idl_conformance.rs` compares this crate's types
//! against `notes/plan/schemas/continuumd-native-protocol.idl`. It can only compare
//! what it can *see*, and a test cannot see a Rust struct's field names — only data.
//! So the types have to describe themselves, and a hand-written description beside a
//! hand-written struct is two texts that can disagree: rename the struct's field,
//! forget the description, and the test still passes against the IDL while the type no
//! longer matches it.
//!
//! [`protocol_struct!`](crate::protocol_struct) closes that hole by deriving both from
//! one token. `overlay` appears once in the macro invocation; the struct field and the
//! [`FieldSpec`] name both come from it. Renaming the field renames the spec, the spec
//! stops matching the IDL, and the test fails naming the operation and the field. There
//! is no edit that changes the type and leaves the description agreeing with the spec.
//!
//! # Presence is three-valued and the types say so
//!
//! > `required` (present, non-null), `nullable` (present, MAY be null — a *named*
//! > absence, INV-007), `optional` (MAY be absent). **Absent and null are distinct and
//! > MUST NOT be conflated** in either direction, by a daemon, a client, or an adapter.
//! >
//! > — RFC 0026, "Versioning and revision"
//!
//! [`Nullable`] and [`Optional`] are therefore distinct types rather than two uses of
//! `Option`. Conflating them is a type error here, not a review obligation.

use core::fmt;

// --- three-valued presence --------------------------------------------------------

/// A `nullable` field: always present on the wire, and MAY be null.
///
/// A null is a *named* absence — "withheld" or "not applicable", stated rather than
/// inferred (INV-007). Omitting a nullable field is malformed, which is why this is not
/// [`Optional`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Nullable<T> {
    /// The field is present and null.
    Null,
    /// The field is present and carries a value.
    Value(T),
}

impl<T> Nullable<T> {
    /// The value, when the field is not null.
    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Null => None,
            Self::Value(value) => Some(value),
        }
    }

    /// Whether the field is null.
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

/// An `optional` field: it MAY be absent, and absent is not null.
///
/// A daemon MUST ignore an unknown optional request field and MUST NOT emit a field the
/// negotiated version does not define (`rule envelope.unknown_fields`). Emitting an
/// optional field as null is malformed, which is why this is not [`Nullable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Optional<T> {
    /// The field is absent from the message.
    Absent,
    /// The field is present and carries a value.
    Present(T),
}

impl<T> Optional<T> {
    /// The value, when the field is present.
    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Absent => None,
            Self::Present(value) => Some(value),
        }
    }

    /// Whether the field is absent.
    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }
}

// --- struct description -----------------------------------------------------------

/// The IDL's three-valued field presence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Presence {
    /// `required`: present and non-null.
    Required,
    /// `nullable`: present, MAY be null.
    Nullable,
    /// `optional`: MAY be absent; absent is not null.
    Optional,
}

impl Presence {
    /// The IDL keyword for this presence.
    #[must_use]
    pub const fn as_idl(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Nullable => "nullable",
            Self::Optional => "optional",
        }
    }
}

impl fmt::Display for Presence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_idl())
    }
}

/// One field of a protocol struct, as the IDL declares it.
///
/// `ty` is the IDL type spelling with interior whitespace removed, so
/// `map<String, Compatibility>` is recorded as `map<String,Compatibility>`. The IDL
/// parser in the conformance test normalizes the same way; the normalization is the
/// only liberty either side takes with the spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSpec {
    /// The wire field name.
    pub name: &'static str,
    /// The IDL type spelling, whitespace-normalized.
    pub ty: &'static str,
    /// The IDL presence marker.
    pub presence: Presence,
}

impl FieldSpec {
    /// Build a field description.
    #[must_use]
    pub const fn new(name: &'static str, ty: &'static str, presence: Presence) -> Self {
        Self { name, ty, presence }
    }
}

/// A struct that carries the IDL declaration it was transcribed from.
pub trait ProtocolStruct {
    /// The IDL struct name — for an anonymous operation body, the generated name.
    const STRUCT_NAME: &'static str;
    /// The fields, in IDL declaration order.
    const FIELDS: &'static [FieldSpec];
}

/// A named struct, its fields, and its codec in each encoding.
///
/// # Why the two closures are here
///
/// The registry names every struct the protocol declares, once, as `StructSpec::of::<T>()`
/// — and that is the only place in the workspace where the *whole* set of declared shapes
/// is enumerated. A conformance sweep over all of them ("every operation's request and
/// response body decodes and re-encodes to itself, in both encodings") therefore has a
/// description of each shape and no way to reach the Rust type it describes, because
/// [`fields`](Self::fields) is data and `T` is not. Restating the 146 type names in a test
/// would answer that with a second list to keep in step, which is the thing this whole
/// module exists to avoid.
///
/// So `of::<T>()` captures `T`'s codec at the same point it captures `T`'s field list:
/// one token, three outputs. A struct that is in the registry is swept, and a struct that
/// is not, is not — there is no way to add one and forget the other.
///
/// [`PartialEq`] deliberately compares only [`name`](Self::name) and
/// [`fields`](Self::fields): the closures are *derived* from the type rather than part of
/// the description, and two descriptions that agree on the declaration agree.
#[derive(Debug, Clone, Copy)]
pub struct StructSpec {
    /// The struct name.
    pub name: &'static str,
    /// The fields, in IDL declaration order.
    pub fields: &'static [FieldSpec],
    /// Decode a canonical JSON document as this struct and re-encode it.
    pub json_round_trip: fn(&crate::codec::json::Json) -> RoundTrip<crate::codec::json::Json>,
    /// Decode a canonical CBOR document as this struct and re-encode it.
    pub cbor_round_trip: fn(&crate::codec::cbor::Cbor) -> RoundTrip<crate::codec::cbor::Cbor>,
}

/// What a [`StructSpec`] round trip answers: the re-encoded document, or why not.
pub type RoundTrip<D> = Result<D, crate::codec::CodecError>;

/// Decode `value` as `T` and encode the result back, in the same encoding.
fn round_trip<D: crate::codec::Document, T: crate::codec::ProtocolValue>(
    value: &D,
) -> RoundTrip<D> {
    T::decode(value)?.encode()
}

impl StructSpec {
    /// The description of `T`, and its codec in each encoding.
    #[must_use]
    pub const fn of<T: ProtocolStruct + crate::codec::ProtocolValue>() -> Self {
        Self {
            name: T::STRUCT_NAME,
            fields: T::FIELDS,
            json_round_trip: round_trip::<crate::codec::json::Json, T>,
            cbor_round_trip: round_trip::<crate::codec::cbor::Cbor, T>,
        }
    }
}

impl PartialEq for StructSpec {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.fields == other.fields
    }
}

impl Eq for StructSpec {}

// --- enum description -------------------------------------------------------------

/// One member of a protocol enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumMember {
    /// The IDL member identifier.
    pub ident: &'static str,
    /// The wire token, which differs from `ident` when the IDL declares one.
    pub wire: &'static str,
}

impl EnumMember {
    /// Build a member description.
    #[must_use]
    pub const fn new(ident: &'static str, wire: &'static str) -> Self {
        Self { ident, wire }
    }
}

/// An enum that carries the IDL declaration it was transcribed from.
pub trait ProtocolEnum: Sized + Copy + PartialEq + 'static {
    /// The IDL enum name.
    const ENUM_NAME: &'static str;
    /// Whether the IDL annotates the enum `@open`.
    const OPEN: bool;
    /// The members, in IDL declaration order.
    const MEMBERS: &'static [EnumMember];
    /// The members as values, in IDL declaration order, index-aligned with
    /// [`MEMBERS`](ProtocolEnum::MEMBERS).
    const ALL: &'static [Self];

    /// This member's wire token.
    fn as_wire(self) -> &'static str;

    /// This member's IDL identifier, which differs from its wire token wherever the IDL
    /// declares one (`revise_intent` against `revise-intent`).
    ///
    /// # Panics
    ///
    /// Never: [`ALL`](ProtocolEnum::ALL) is generated from the same repetition as
    /// [`MEMBERS`](ProtocolEnum::MEMBERS), so every value has an index in it.
    fn ident(self) -> &'static str {
        let index = Self::ALL
            .iter()
            .position(|member| *member == self)
            .expect("ALL and MEMBERS are generated from one declaration");
        Self::MEMBERS[index].ident
    }

    /// Decode a wire token, failing closed on anything the version does not define.
    ///
    /// A client receiving an unknown member of a *closed* enum MUST treat the message as
    /// malformed (`rule versioning.enums`); that is this [`Err`]. An `@open` enum is
    /// decoded through [`Open::decode`] instead, which never fails.
    ///
    /// # Errors
    ///
    /// [`UnknownMember`] when no member carries the token.
    fn from_wire(token: &str) -> Result<Self, UnknownMember> {
        Self::MEMBERS
            .iter()
            .position(|member| member.wire == token)
            .map(|index| Self::ALL[index])
            .ok_or_else(|| UnknownMember::new(Self::ENUM_NAME, token))
    }
}

/// A named enum and its members.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumSpec {
    /// The enum name.
    pub name: &'static str,
    /// Whether the IDL annotates it `@open`.
    pub open: bool,
    /// The members, in IDL declaration order.
    pub members: &'static [EnumMember],
}

impl EnumSpec {
    /// The description of `T`.
    #[must_use]
    pub const fn of<T: ProtocolEnum>() -> Self {
        Self {
            name: T::ENUM_NAME,
            open: T::OPEN,
            members: T::MEMBERS,
        }
    }
}

/// A rejected wire token.
///
/// [`Display`](fmt::Display) names the enum and never the token: the token came off the
/// wire, and a typed error's `detail` is "stable, non-interpolated" (INV-016,
/// `rule envelope.no_prose`). A caller that needs the token for typed handling reads
/// [`token`](UnknownMember::token) and MUST NOT interpolate it into prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownMember {
    enum_name: &'static str,
    token: Box<str>,
}

impl UnknownMember {
    /// Record a rejection of `token` against the enum named `enum_name`.
    #[must_use]
    pub fn new(enum_name: &'static str, token: &str) -> Self {
        Self {
            enum_name,
            token: token.into(),
        }
    }

    /// The enum the token was offered to.
    #[must_use]
    pub const fn enum_name(&self) -> &'static str {
        self.enum_name
    }

    /// The rejected token, verbatim. Never interpolate it into prose.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }
}

impl fmt::Display for UnknownMember {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unrecognized member of the closed enum `{}`",
            self.enum_name
        )
    }
}

impl core::error::Error for UnknownMember {}

/// The decoded form of an `@open` enum.
///
/// > A client receiving an unknown member of an `@open` enum MUST NOT infer semantics
/// > from it, MUST NOT crash, and MUST surface it verbatim to its caller as unknown.
/// >
/// > — `rule versioning.enums`
///
/// [`Unknown`](Open::Unknown) is that surface. It is deliberately not convertible into
/// the known variant: there is no "treat it as the closest member" path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Open<T> {
    /// A member this version defines.
    Known(T),
    /// A token this version does not define, carried verbatim.
    Unknown(Box<str>),
}

impl<T: ProtocolEnum> Open<T> {
    /// Decode a wire token. Never fails: an unrecognized token becomes
    /// [`Unknown`](Open::Unknown).
    ///
    /// # Panics
    ///
    /// Never. Decoding an `@open` enum has no failure mode by construction.
    #[must_use]
    pub fn decode(token: &str) -> Self {
        assert!(
            T::OPEN,
            "Open<T> decodes @open enums only; a closed enum fails closed through from_wire"
        );
        T::from_wire(token).map_or_else(|_| Self::Unknown(token.into()), Self::Known)
    }

    /// The known member, when the token was one this version defines.
    pub const fn known(&self) -> Option<&T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unknown(_) => None,
        }
    }

    /// The verbatim token, when this version does not define it.
    pub fn unknown(&self) -> Option<&str> {
        match self {
            Self::Known(_) => None,
            Self::Unknown(token) => Some(token),
        }
    }
}

// --- union description ------------------------------------------------------------

/// One variant of a protocol union.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnionVariant {
    /// The IDL variant identifier.
    pub ident: &'static str,
    /// The IDL type spelling the variant carries.
    pub ty: &'static str,
}

impl UnionVariant {
    /// Build a variant description.
    #[must_use]
    pub const fn new(ident: &'static str, ty: &'static str) -> Self {
        Self { ident, ty }
    }
}

/// A union that carries the IDL declaration it was transcribed from.
pub trait ProtocolUnion {
    /// The IDL union name.
    const UNION_NAME: &'static str;
    /// The variants, in IDL declaration order.
    const VARIANTS: &'static [UnionVariant];
}

/// A named union and its variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnionSpec {
    /// The union name.
    pub name: &'static str,
    /// The variants, in IDL declaration order.
    pub variants: &'static [UnionVariant],
}

impl UnionSpec {
    /// The description of `T`.
    #[must_use]
    pub const fn of<T: ProtocolUnion>() -> Self {
        Self {
            name: T::UNION_NAME,
            variants: T::VARIANTS,
        }
    }
}

// --- handle and alias description --------------------------------------------------

/// A handle class and the prefix its wire spelling carries.
pub trait ProtocolHandle {
    /// The IDL handle name.
    const HANDLE_NAME: &'static str;
    /// The class prefix, including the trailing underscore.
    const PREFIX: &'static str;
}

/// A named handle class and its prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleSpec {
    /// The handle name.
    pub name: &'static str,
    /// The class prefix.
    pub prefix: &'static str,
}

impl HandleSpec {
    /// The description of `T`.
    #[must_use]
    pub const fn of<T: ProtocolHandle>() -> Self {
        Self {
            name: T::HANDLE_NAME,
            prefix: T::PREFIX,
        }
    }
}

/// A string alias and the `@pattern` constraining it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AliasSpec {
    /// The alias name.
    pub name: &'static str,
    /// The base type it aliases.
    pub base: &'static str,
    /// The `@pattern` regular expression, when the IDL declares one.
    pub pattern: Option<&'static str>,
}

// --- operation description ---------------------------------------------------------

/// An IDL annotation on an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Annotation {
    /// `@readonly`: performs no state change; needs no idempotency key.
    Readonly,
    /// `@mutation`: changes daemon state; the request MUST carry `idempotency_key`.
    Mutation,
    /// `@task_starting`: MAY return `task_started` with a task handle.
    TaskStarting,
    /// `@privileged`: privileged operation; audit-recorded (plan §18.5).
    Privileged,
    /// `@audit_recorded`: every call is written to the audit log.
    AuditRecorded,
    /// `@paginated`: accepts `page`, returns `next_page_token`.
    Paginated,
    /// `@streaming`: long-lived; delivers `events` frames on the same connection.
    Streaming,
}

impl Annotation {
    /// The IDL spelling, without the leading `@`.
    #[must_use]
    pub const fn as_idl(self) -> &'static str {
        match self {
            Self::Readonly => "readonly",
            Self::Mutation => "mutation",
            Self::TaskStarting => "task_starting",
            Self::Privileged => "privileged",
            Self::AuditRecorded => "audit_recorded",
            Self::Paginated => "paginated",
            Self::Streaming => "streaming",
        }
    }
}

impl fmt::Display for Annotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_idl())
    }
}

/// One operation of the plan §10.2 registry, as the IDL declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationSpec {
    /// The wire name, `namespace.verb`.
    pub name: &'static str,
    /// The minimum authority level, mirroring RFC 0027's registry table.
    pub authority: crate::protocol::vocabulary::AuthorityLevel,
    /// The IDL annotations, in declaration order.
    pub annotations: &'static [Annotation],
    /// The request body.
    pub request: StructSpec,
    /// The response body.
    pub response: StructSpec,
    /// The `verdict` clause's value type, when the operation declares one.
    pub verdict: Option<&'static str>,
    /// The `events` clause's type, when the operation declares one.
    pub events: Option<&'static str>,
    /// The `errors` clause: the codes this operation may return *beyond* the common
    /// union of `rule errors.common`. An empty clause means "adds nothing", never
    /// "cannot fail" (RFC 0026 correction 15).
    pub errors: &'static [crate::protocol::vocabulary::ErrorCode],
}

impl OperationSpec {
    /// Whether the operation carries `annotation`.
    #[must_use]
    pub fn has(&self, annotation: Annotation) -> bool {
        self.annotations.contains(&annotation)
    }
}

// --- the macros --------------------------------------------------------------------

/// Emit a protocol struct, the [`FieldSpec`] list describing it, and its codec, from one
/// text.
///
/// The body is the IDL's own field syntax: `name: type presence;`, with `list<T>` and
/// `map<String, T>` written as the IDL writes them. See the module documentation for why
/// all three outputs come from one declaration.
///
/// # Why the codec is emitted here too
///
/// The same argument that makes [`FieldSpec`] macro-generated makes the codec
/// macro-generated: a hand-written encoder beside a hand-written struct is two texts that
/// can disagree, and the disagreement is invisible — a field written under the wrong key,
/// or silently not written at all, is a message that still parses. Here the wire key, the
/// struct field, and the `FieldSpec` name all come from one token, so a rename moves all
/// three or none.
///
/// The presence marker reaches the codec the same way: it selects which of the nine
/// `put_*`/`take_*` helpers a field uses, so "absent and null are distinct in both
/// directions" is a property of the declaration rather than of nine hand-written matches.
#[macro_export]
macro_rules! protocol_struct {
    (
        $(#[doc = $struct_doc:literal])*
        struct $name:ident { $($body:tt)* }
    ) => {
        $crate::protocol_struct!(
            @munch $name { $(#[doc = $struct_doc])* } { } { } { } $($body)*
        );
    };

    // --- terminal: emit the struct, its description, and its codec ---
    //
    // The codec is generated *here*, from an accumulated `field @ mode` list, rather than
    // accumulated as expressions in the arms below: `self` and the field map are bound in
    // this arm, and macro hygiene keeps tokens produced by a different arm from naming
    // them. The mode token is what carries the presence marker across.
    (@munch $name:ident { $($sdoc:tt)* } { $($fields:tt)* } { $($specs:tt)* }
        { $($member:ident @ $mode:ident)* }) => {
        $($sdoc)*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name { $($fields)* }

        impl $crate::protocol::spec::ProtocolStruct for $name {
            const STRUCT_NAME: &'static str = stringify!($name);
            const FIELDS: &'static [$crate::protocol::spec::FieldSpec] = &[$($specs)*];
        }

        impl $crate::codec::ProtocolValue for $name {
            fn encode<D: $crate::codec::Document>(
                &self,
            ) -> ::core::result::Result<D, $crate::codec::CodecError> {
                // A body with no fields (`signing.registry`'s request) inserts nothing.
                #[allow(unused_mut)]
                let mut into = ::std::collections::BTreeMap::new();
                $($crate::__protocol_put!(
                    $mode, into, stringify!($member), &self.$member);)*
                ::core::result::Result::Ok(<D as $crate::codec::Document>::from_entries(into))
            }

            fn decode<D: $crate::codec::Document>(
                value: &D,
            ) -> ::core::result::Result<Self, $crate::codec::CodecError> {
                // Checked even for a body with no fields: it must still be an object.
                #[allow(unused_variables)]
                let from = $crate::codec::object_of(value, stringify!($name))?;
                // Unknown keys are ignored rather than rejected: "a daemon MUST ignore
                // unknown `optional` request fields" (`rule envelope.unknown_fields`) is
                // this protocol's only forward-compatibility mechanism, and rejecting them
                // would make every compatible minor breaking for an older reader.
                ::core::result::Result::Ok(Self {
                    $($member: $crate::__protocol_take!(
                        $mode, from, stringify!($name), stringify!($member))?,)*
                })
            }
        }
    };

    // --- plain type ---
    (@munch $name:ident { $($sdoc:tt)* } { $($fields:tt)* } { $($specs:tt)* }
        { $($codec:tt)* }
        $(#[doc = $fdoc:literal])* $field:ident : $ty:ident required ; $($rest:tt)*
    ) => {
        $crate::protocol_struct!(@munch $name { $($sdoc)* }
            { $($fields)* $(#[doc = $fdoc])* pub $field : $ty , }
            { $($specs)* $crate::protocol::spec::FieldSpec::new(
                stringify!($field), stringify!($ty),
                $crate::protocol::spec::Presence::Required), }
            { $($codec)* $field @ required }
            $($rest)*);
    };
    (@munch $name:ident { $($sdoc:tt)* } { $($fields:tt)* } { $($specs:tt)* }
        { $($codec:tt)* }
        $(#[doc = $fdoc:literal])* $field:ident : $ty:ident nullable ; $($rest:tt)*
    ) => {
        $crate::protocol_struct!(@munch $name { $($sdoc)* }
            { $($fields)* $(#[doc = $fdoc])* pub $field :
                $crate::protocol::spec::Nullable<$ty> , }
            { $($specs)* $crate::protocol::spec::FieldSpec::new(
                stringify!($field), stringify!($ty),
                $crate::protocol::spec::Presence::Nullable), }
            { $($codec)* $field @ nullable }
            $($rest)*);
    };
    (@munch $name:ident { $($sdoc:tt)* } { $($fields:tt)* } { $($specs:tt)* }
        { $($codec:tt)* }
        $(#[doc = $fdoc:literal])* $field:ident : $ty:ident optional ; $($rest:tt)*
    ) => {
        $crate::protocol_struct!(@munch $name { $($sdoc)* }
            { $($fields)* $(#[doc = $fdoc])* pub $field :
                $crate::protocol::spec::Optional<$ty> , }
            { $($specs)* $crate::protocol::spec::FieldSpec::new(
                stringify!($field), stringify!($ty),
                $crate::protocol::spec::Presence::Optional), }
            { $($codec)* $field @ optional }
            $($rest)*);
    };

    // --- list<T> ---
    (@munch $name:ident { $($sdoc:tt)* } { $($fields:tt)* } { $($specs:tt)* }
        { $($codec:tt)* }
        $(#[doc = $fdoc:literal])* $field:ident : list < $ty:ident > required ; $($rest:tt)*
    ) => {
        $crate::protocol_struct!(@munch $name { $($sdoc)* }
            { $($fields)* $(#[doc = $fdoc])* pub $field : ::std::vec::Vec<$ty> , }
            { $($specs)* $crate::protocol::spec::FieldSpec::new(
                stringify!($field), concat!("list<", stringify!($ty), ">"),
                $crate::protocol::spec::Presence::Required), }
            { $($codec)* $field @ required_list }
            $($rest)*);
    };
    (@munch $name:ident { $($sdoc:tt)* } { $($fields:tt)* } { $($specs:tt)* }
        { $($codec:tt)* }
        $(#[doc = $fdoc:literal])* $field:ident : list < $ty:ident > nullable ; $($rest:tt)*
    ) => {
        $crate::protocol_struct!(@munch $name { $($sdoc)* }
            { $($fields)* $(#[doc = $fdoc])* pub $field :
                $crate::protocol::spec::Nullable<::std::vec::Vec<$ty>> , }
            { $($specs)* $crate::protocol::spec::FieldSpec::new(
                stringify!($field), concat!("list<", stringify!($ty), ">"),
                $crate::protocol::spec::Presence::Nullable), }
            { $($codec)* $field @ nullable_list }
            $($rest)*);
    };
    (@munch $name:ident { $($sdoc:tt)* } { $($fields:tt)* } { $($specs:tt)* }
        { $($codec:tt)* }
        $(#[doc = $fdoc:literal])* $field:ident : list < $ty:ident > optional ; $($rest:tt)*
    ) => {
        $crate::protocol_struct!(@munch $name { $($sdoc)* }
            { $($fields)* $(#[doc = $fdoc])* pub $field :
                $crate::protocol::spec::Optional<::std::vec::Vec<$ty>> , }
            { $($specs)* $crate::protocol::spec::FieldSpec::new(
                stringify!($field), concat!("list<", stringify!($ty), ">"),
                $crate::protocol::spec::Presence::Optional), }
            { $($codec)* $field @ optional_list }
            $($rest)*);
    };

    // --- map<String, T> ---
    (@munch $name:ident { $($sdoc:tt)* } { $($fields:tt)* } { $($specs:tt)* }
        { $($codec:tt)* }
        $(#[doc = $fdoc:literal])* $field:ident : map < $key:ident , $ty:ident > required ;
        $($rest:tt)*
    ) => {
        $crate::protocol_struct!(@munch $name { $($sdoc)* }
            { $($fields)* $(#[doc = $fdoc])* pub $field :
                ::std::collections::BTreeMap<$key, $ty> , }
            { $($specs)* $crate::protocol::spec::FieldSpec::new(
                stringify!($field),
                concat!("map<", stringify!($key), ",", stringify!($ty), ">"),
                $crate::protocol::spec::Presence::Required), }
            { $($codec)* $field @ required_map }
            $($rest)*);
    };
    (@munch $name:ident { $($sdoc:tt)* } { $($fields:tt)* } { $($specs:tt)* }
        { $($codec:tt)* }
        $(#[doc = $fdoc:literal])* $field:ident : map < $key:ident , $ty:ident > nullable ;
        $($rest:tt)*
    ) => {
        $crate::protocol_struct!(@munch $name { $($sdoc)* }
            { $($fields)* $(#[doc = $fdoc])* pub $field :
                $crate::protocol::spec::Nullable<::std::collections::BTreeMap<$key, $ty>> , }
            { $($specs)* $crate::protocol::spec::FieldSpec::new(
                stringify!($field),
                concat!("map<", stringify!($key), ",", stringify!($ty), ">"),
                $crate::protocol::spec::Presence::Nullable), }
            { $($codec)* $field @ nullable_map }
            $($rest)*);
    };
    (@munch $name:ident { $($sdoc:tt)* } { $($fields:tt)* } { $($specs:tt)* }
        { $($codec:tt)* }
        $(#[doc = $fdoc:literal])* $field:ident : map < $key:ident , $ty:ident > optional ;
        $($rest:tt)*
    ) => {
        $crate::protocol_struct!(@munch $name { $($sdoc)* }
            { $($fields)* $(#[doc = $fdoc])* pub $field :
                $crate::protocol::spec::Optional<::std::collections::BTreeMap<$key, $ty>> , }
            { $($specs)* $crate::protocol::spec::FieldSpec::new(
                stringify!($field),
                concat!("map<", stringify!($key), ",", stringify!($ty), ">"),
                $crate::protocol::spec::Presence::Optional), }
            { $($codec)* $field @ optional_map }
            $($rest)*);
    };
}

/// Dispatch a field's presence-and-shape mode to the codec helper that writes it.
#[macro_export]
#[doc(hidden)]
macro_rules! __protocol_put {
    (required, $into:ident, $name:expr, $value:expr) => {
        $crate::codec::put_required(&mut $into, $name, $value)?
    };
    (nullable, $into:ident, $name:expr, $value:expr) => {
        $crate::codec::put_nullable(&mut $into, $name, $value)?
    };
    (optional, $into:ident, $name:expr, $value:expr) => {
        $crate::codec::put_optional(&mut $into, $name, $value)?
    };
    (required_list, $into:ident, $name:expr, $value:expr) => {
        $crate::codec::put_required_list(&mut $into, $name, $value)?
    };
    (nullable_list, $into:ident, $name:expr, $value:expr) => {
        $crate::codec::put_nullable_list(&mut $into, $name, $value)?
    };
    (optional_list, $into:ident, $name:expr, $value:expr) => {
        $crate::codec::put_optional_list(&mut $into, $name, $value)?
    };
    (required_map, $into:ident, $name:expr, $value:expr) => {
        $crate::codec::put_required_map(&mut $into, $name, $value)?
    };
    (nullable_map, $into:ident, $name:expr, $value:expr) => {
        $crate::codec::put_nullable_map(&mut $into, $name, $value)?
    };
    (optional_map, $into:ident, $name:expr, $value:expr) => {
        $crate::codec::put_optional_map(&mut $into, $name, $value)?
    };
}

/// Dispatch a field's presence-and-shape mode to the codec helper that reads it.
#[macro_export]
#[doc(hidden)]
macro_rules! __protocol_take {
    (required, $from:ident, $declared:expr, $name:expr) => {
        $crate::codec::take_required($from, $declared, $name)
    };
    (nullable, $from:ident, $declared:expr, $name:expr) => {
        $crate::codec::take_nullable($from, $declared, $name)
    };
    (optional, $from:ident, $declared:expr, $name:expr) => {
        $crate::codec::take_optional($from, $declared, $name)
    };
    (required_list, $from:ident, $declared:expr, $name:expr) => {
        $crate::codec::take_required_list($from, $declared, $name)
    };
    (nullable_list, $from:ident, $declared:expr, $name:expr) => {
        $crate::codec::take_nullable_list($from, $declared, $name)
    };
    (optional_list, $from:ident, $declared:expr, $name:expr) => {
        $crate::codec::take_optional_list($from, $declared, $name)
    };
    (required_map, $from:ident, $declared:expr, $name:expr) => {
        $crate::codec::take_required_map($from, $declared, $name)
    };
    (nullable_map, $from:ident, $declared:expr, $name:expr) => {
        $crate::codec::take_nullable_map($from, $declared, $name)
    };
    (optional_map, $from:ident, $declared:expr, $name:expr) => {
        $crate::codec::take_optional_map($from, $declared, $name)
    };
}

/// Expand to a member's wire token: the declared literal, or the identifier's spelling.
#[macro_export]
#[doc(hidden)]
macro_rules! __protocol_wire_token {
    ($ident:ident) => {
        stringify!($ident)
    };
    ($ident:ident, $wire:literal) => {
        $wire
    };
}

/// Expand to whether an enum is `@open`.
#[macro_export]
#[doc(hidden)]
macro_rules! __protocol_openness {
    (open) => {
        true
    };
    (closed) => {
        false
    };
}

/// Emit a protocol enum, its wire tokens, and the [`EnumSpec`] describing it.
///
/// The `open`/`closed` keyword records the IDL's `@open` annotation, which decides
/// whether an unrecognized token is malformed or surfaced verbatim
/// (`rule versioning.enums`).
#[macro_export]
macro_rules! protocol_enum {
    (
        $(#[doc = $enum_doc:literal])*
        $openness:ident enum $name:ident {
            $(
                $(#[doc = $member_doc:literal])*
                $member:ident $(= $wire:literal)? => $variant:ident ,
            )*
        }
    ) => {
        $(#[doc = $enum_doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $(
                $(#[doc = $member_doc])*
                $variant ,
            )*
        }

        impl $crate::protocol::spec::ProtocolEnum for $name {
            const ENUM_NAME: &'static str = stringify!($name);
            const OPEN: bool = $crate::__protocol_openness!($openness);
            const MEMBERS: &'static [$crate::protocol::spec::EnumMember] = &[
                $($crate::protocol::spec::EnumMember::new(
                    stringify!($member),
                    $crate::__protocol_wire_token!($member $(, $wire)?),
                ),)*
            ];
            const ALL: &'static [Self] = &[$(Self::$variant,)*];

            fn as_wire(self) -> &'static str {
                match self {
                    $(Self::$variant => $crate::__protocol_wire_token!($member $(, $wire)?),)*
                }
            }
        }

        impl $crate::codec::ProtocolValue for $name {
            fn encode<D: $crate::codec::Document>(
                &self,
            ) -> ::core::result::Result<D, $crate::codec::CodecError> {
                ::core::result::Result::Ok(<D as $crate::codec::Document>::from_text(
                    <Self as $crate::protocol::spec::ProtocolEnum>::as_wire(*self),
                ))
            }

            fn decode<D: $crate::codec::Document>(
                value: &D,
            ) -> ::core::result::Result<Self, $crate::codec::CodecError> {
                let token = <D as $crate::codec::Document>::as_text(value).ok_or(
                    $crate::codec::CodecError::TypeMismatch {
                        expected: stringify!($name),
                        found: <D as $crate::codec::Document>::kind(value),
                    },
                )?;
                // A closed enum fails closed and an `@open` one surfaces the token
                // verbatim — the two halves of `rule versioning.enums`, decided from the
                // `@open` annotation the declaration already carries rather than from a
                // second list of which enums are open.
                <Self as $crate::protocol::spec::ProtocolEnum>::from_wire(token).map_err(|_| {
                    if <Self as $crate::protocol::spec::ProtocolEnum>::OPEN {
                        $crate::codec::CodecError::UnknownOpenMember {
                            enum_name: stringify!($name),
                            token: token.into(),
                        }
                    } else {
                        $crate::codec::CodecError::UnknownMember {
                            enum_name: stringify!($name),
                        }
                    }
                })
            }
        }
    };
}

/// Emit a protocol union and the [`UnionSpec`] describing it.
#[macro_export]
macro_rules! protocol_union {
    (
        $(#[doc = $union_doc:literal])*
        union $name:ident {
            $(
                $(#[doc = $variant_doc:literal])*
                $member:ident ( $ty:ident ) => $variant:ident ,
            )*
        }
    ) => {
        $(#[doc = $union_doc])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum $name {
            $(
                $(#[doc = $variant_doc])*
                $variant($ty),
            )*
        }

        impl $crate::protocol::spec::ProtocolUnion for $name {
            const UNION_NAME: &'static str = stringify!($name);
            const VARIANTS: &'static [$crate::protocol::spec::UnionVariant] = &[
                $($crate::protocol::spec::UnionVariant::new(
                    stringify!($member), stringify!($ty),
                ),)*
            ];
        }

        impl $crate::codec::ProtocolValue for $name {
            fn encode<D: $crate::codec::Document>(
                &self,
            ) -> ::core::result::Result<D, $crate::codec::CodecError> {
                // `rule encoding.union_tagging`: externally tagged, one key, the variant
                // identifier the IDL declares.
                let mut object = ::std::collections::BTreeMap::new();
                match self {
                    $(Self::$variant(inner) => {
                        object.insert(
                            stringify!($member).to_owned(),
                            $crate::codec::ProtocolValue::encode::<D>(inner)?,
                        );
                    })*
                }
                ::core::result::Result::Ok(<D as $crate::codec::Document>::from_entries(object))
            }

            fn decode<D: $crate::codec::Document>(
                value: &D,
            ) -> ::core::result::Result<Self, $crate::codec::CodecError> {
                let object = $crate::codec::object_of(value, stringify!($name))?;
                let mut entries = object.iter();
                let (tag, inner) = match (entries.next(), entries.next()) {
                    (::core::option::Option::Some(first), ::core::option::Option::None) => first,
                    _ => {
                        return ::core::result::Result::Err(
                            $crate::codec::CodecError::UnionArity {
                                union: stringify!($name),
                            },
                        );
                    }
                };
                match tag.as_str() {
                    $(stringify!($member) => ::core::result::Result::Ok(Self::$variant(
                        <$ty as $crate::codec::ProtocolValue>::decode(inner)?,
                    )),)*
                    // A union is closed, so an unrecognized variant key fails closed
                    // rather than being surfaced: there is no `@open` union in the IDL.
                    _ => ::core::result::Result::Err($crate::codec::CodecError::UnknownVariant {
                        union: stringify!($name),
                    }),
                }
            }
        }
    };
}
