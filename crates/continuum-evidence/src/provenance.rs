//! Provenance: who produced a node or an edge, from what inputs, at what time, and under
//! which epochs (PR-7 / IMPL-02).
//!
//! # What the dossier asks for, member by member
//!
//! > Every node records actor/tool/model version, prompts/tool inputs as policy permits,
//! > source retrievals, and derivation. This supports scientific credit, debugging,
//! > benchmark analysis, and reproduction without granting authority based on identity.
//! >
//! > — docs/44, "Credit and provenance"
//!
//! RFC 0038 has no prose "provenance section"; it delegates — "This RFC summarizes; the
//! schemas decide" — and both schemas carry the same object:
//!
//! ```text
//! "provenance": {
//!   "additionalProperties": false,
//!   "properties": {
//!     "actor":      { "type": "string" },
//!     "created_at": { "type": "string", "format": "date-time" },
//!     "inputs":     { "items": { "pattern": "^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$" } },
//!     "tool":       { "type": "string" }
//!   },
//!   "required": ["actor", "created_at", "inputs"]
//! }
//! ```
//!
//! — `schemas/evidence-graph-node.schema.json` and `evidence-graph-edge.schema.json`
//!
//! So the *rendered* record is exactly four members, three of them mandatory, and
//! `additionalProperties: false` makes a fifth rendered member as invalid as a missing
//! third (GOV-1-12: ambiguity is an error, and a schema that closes its top level says
//! which fields exist). [`Provenance::to_record`] emits exactly those, and
//! `record_is_exactly_the_schema_object` holds it there.
//!
//! # Where the epochs go, and why they are not in that object
//!
//! > evidence is epoch-scoped: a receipt remains verifiable under its pinned epoch
//! > indefinitely (INV-006, INV-014); an epoch advance never silently revalidates or
//! > invalidates a published receipt
//! >
//! > — plan §4.6
//!
//! A record that cannot say which epochs a write happened under cannot support that
//! sentence, and neither schema has a member for it — the provenance object is closed at
//! four and the node object is closed at its own list. Both readings would be wrong:
//! dropping the epochs loses plan §4.6, and rendering them inside `provenance` writes a
//! field the schema forbids.
//!
//! [`Provenance`] therefore *carries* an [`EpochSet`] and does not *render* it. The
//! library record is the stronger object; the wire record is the schema's four members,
//! and `epochs_are_carried_and_never_rendered` asserts both halves. Which envelope the
//! epochs ride in on the wire is RFC 0026's question (its result envelope already has an
//! `epochs` object) and is not decided here.
//!
//! # Absent is a value, not a silence
//!
//! `tool` is the schema's one optional member and is [`Option`] here. Everything else is
//! required by the schema and is a plain field, so a provenance-free node or edge is not
//! something this crate refuses at run time — it is something a caller cannot express.
//! That is the structural half of PR-7 / IMPL-02: [`crate::node::EvidenceNode`] and
//! [`crate::edge::EvidenceEdge`] take a `Provenance` by value in their only constructors,
//! so "the graph refuses provenance-free additions" is a fact about the types rather than
//! a validation anything performs.
//!
//! `inputs` is a [`BTreeSet`] rather than a list. An artifact either is an input to this
//! derivation or is not; listing one twice, or in two orders, describes one derivation two
//! ways, and two spellings of one record would give it two content identities. The
//! rendering is therefore canonically ascending and duplicate-free by construction.
//!
//! # Time is an input, never a reading
//!
//! Nothing in this crate reads a clock. `created_at` is a [`Timestamp`] the caller supplies,
//! in the one spelling the wire fixes (`YYYY-MM-DDTHH:MM:SS.sssZ`), because ambient time is
//! exactly what INV-005 and ADR-0003 forbid and GOV-1-04 checks for mechanically. The
//! daemon already models this: `observe.ingest` refuses to append at all when the
//! deployment holds no time capability, rather than stamping a node with a time it invented.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | actor, created_at, inputs required; tool optional | both schemas | `record_is_exactly_the_schema_object`, `a_tool_less_record_omits_the_member` |
//! | `additionalProperties: false` | both schemas | `record_is_exactly_the_schema_object` |
//! | input handles match the artifact pattern | both schemas | `input_handles_match_the_artifact_pattern` |
//! | `created_at` is `date-time` | both schemas | `timestamps_are_the_wire_spelling` |
//! | evidence is epoch-scoped | plan §4.6 | `epochs_are_carried_and_never_rendered` |
//! | provenance is structural, not validated | PR-7 / IMPL-02 | `crate::node::tests::a_node_cannot_be_built_without_provenance` (compile-fail doc test) |
//! | one derivation has one spelling | ADR-0013 | `inputs_are_a_set`, `provenance_value_is_order_independent` |

use core::fmt;
use std::collections::BTreeSet;

use continuum_value::epoch::{EpochKind, EpochSet};
use continuum_value::value::{Name, Value};

use crate::actor::{ActorId, field};

/// An artifact handle as the evidence schemas spell one.
///
/// > `"pattern": "^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$"`
/// >
/// > — `evidence-graph-node.schema.json`, `artifact` and `provenance.inputs[]`
///
/// A class prefix, an underscore, and an identity: `trace_9f…`, `ev_1a…`, `crash_c0…`. The
/// pattern is the schemas' and not this module's, so a handle that round-trips here is a
/// handle the graph could publish.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactRef(String);

impl ArtifactRef {
    /// The pattern this type accepts, in the schemas' own spelling.
    pub const PATTERN: &'static str = "^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$";

    /// Parse an artifact handle.
    ///
    /// # Errors
    ///
    /// [`ArtifactRefError`] when the class prefix is empty or not `[a-z][a-z0-9_]*`, when
    /// there is no separating underscore, or when the identity half is empty or carries a
    /// character outside `[A-Za-z0-9_-]`.
    pub fn new(text: &str) -> Result<Self, ArtifactRefError> {
        let (class, identity) = text.rsplit_once('_').ok_or(ArtifactRefError::NoSeparator)?;
        let mut class_bytes = class.bytes();
        match class_bytes.next() {
            Some(byte) if byte.is_ascii_lowercase() => {}
            Some(_) => return Err(ArtifactRefError::ClassCharacter),
            None => return Err(ArtifactRefError::EmptyClass),
        }
        if !class_bytes
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(ArtifactRefError::ClassCharacter);
        }
        if identity.is_empty() {
            return Err(ArtifactRefError::EmptyIdentity);
        }
        if !identity
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(ArtifactRefError::IdentityCharacter);
        }
        Ok(Self(text.to_owned()))
    }

    /// The canonical spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// This handle as a canonical value, for a content-identity preimage.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::text(self.0.clone())
    }
}

impl fmt::Display for ArtifactRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why an artifact handle was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArtifactRefError {
    /// No `_` separates the class prefix from the identity.
    NoSeparator,
    /// The class prefix is empty.
    EmptyClass,
    /// The class prefix carries a character outside `[a-z][a-z0-9_]*`.
    ClassCharacter,
    /// The identity half is empty.
    EmptyIdentity,
    /// The identity half carries a character outside `[A-Za-z0-9_-]`.
    IdentityCharacter,
}

impl fmt::Display for ArtifactRefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NoSeparator => "an artifact handle is `class_identity` and carries no `_`",
            Self::EmptyClass => "the class prefix of an artifact handle is empty",
            Self::ClassCharacter => "the class prefix carries a character outside [a-z][a-z0-9_]*",
            Self::EmptyIdentity => "the identity half of an artifact handle is empty",
            Self::IdentityCharacter => {
                "the identity half carries a character outside [A-Za-z0-9_-]"
            }
        };
        f.write_str(message)
    }
}

impl core::error::Error for ArtifactRefError {}

/// A `date-time` in the one spelling the wire fixes, `YYYY-MM-DDTHH:MM:SS.sssZ`.
///
/// The shape and the field ranges are the daemon's `Timestamp` scalar, transcribed rather
/// than reinvented so a provenance record written here is one the protocol accepts.
/// Calendar validity is deliberately *not* checked: neither schema states a calendar rule,
/// and a rejection this type invented would be one no producer could have predicted.
///
/// There is no constructor that reads a clock. Time is an explicit input (INV-005,
/// ADR-0003, GOV-1-04).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(String);

impl Timestamp {
    /// The canonical spelling this type accepts.
    pub const SHAPE: &'static str = "YYYY-MM-DDTHH:MM:SS.sssZ";

    /// Parse a timestamp.
    ///
    /// # Errors
    ///
    /// [`TimestampError::NotCanonical`] when the text is not [`SHAPE`](Self::SHAPE), and
    /// [`TimestampError::OutOfRange`] when a field is outside month `01`-`12`, day
    /// `01`-`31`, hour `00`-`23`, minute `00`-`59`, second `00`-`60` (a leap second is a
    /// real second).
    pub fn new(text: &str) -> Result<Self, TimestampError> {
        let bytes = text.as_bytes();
        if bytes.len() != 24 {
            return Err(TimestampError::NotCanonical);
        }
        for (index, expected) in b"____-__-__T__:__:__.___Z".iter().enumerate() {
            let actual = bytes[index];
            let ok = match expected {
                b'_' => actual.is_ascii_digit(),
                other => actual == *other,
            };
            if !ok {
                return Err(TimestampError::NotCanonical);
            }
        }
        let field_at = |from: usize, to: usize| -> u32 {
            text[from..to]
                .parse()
                .expect("the shape check accepted only ASCII digits here")
        };
        let (month, day) = (field_at(5, 7), field_at(8, 10));
        let (hour, minute, second) = (field_at(11, 13), field_at(14, 16), field_at(17, 19));
        if !(1..=12).contains(&month)
            || !(1..=31).contains(&day)
            || hour > 23
            || minute > 59
            || second > 60
        {
            return Err(TimestampError::OutOfRange);
        }
        Ok(Self(text.to_owned()))
    }

    /// The canonical spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// This timestamp as a canonical value, for a content-identity preimage.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::text(self.0.clone())
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a timestamp was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TimestampError {
    /// The text is not `YYYY-MM-DDTHH:MM:SS.sssZ`.
    NotCanonical,
    /// A field is out of range.
    OutOfRange,
}

impl fmt::Display for TimestampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotCanonical => "a timestamp is spelled YYYY-MM-DDTHH:MM:SS.sssZ",
            Self::OutOfRange => "a timestamp field is out of range",
        };
        f.write_str(message)
    }
}

impl core::error::Error for TimestampError {}

/// The tool or model version that produced an artifact (docs/44: "actor/tool/model
/// version").
///
/// The schemas type it as a bare string; this newtype adds only non-emptiness, because an
/// empty tool name is a rendered field that says nothing, which is the "empty success"
/// shape the dossier rejects everywhere else.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tool(String);

impl Tool {
    /// Name a tool.
    ///
    /// # Errors
    ///
    /// [`EmptyTool`] for the empty string.
    pub fn new(text: &str) -> Result<Self, EmptyTool> {
        if text.is_empty() {
            return Err(EmptyTool);
        }
        Ok(Self(text.to_owned()))
    }

    /// The tool's name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A tool was named with the empty string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct EmptyTool;

impl fmt::Display for EmptyTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a tool name is not the empty string")
    }
}

impl core::error::Error for EmptyTool {}

/// Who produced an evidence node or edge, from what, when, and under which epochs.
///
/// Immutable once built: every accessor is a borrow and there is no setter, because the
/// graph is append-only and a provenance record that could be edited would be a record of
/// the last edit rather than of the production (plan §11.7, RFC 0038).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    actor: ActorId,
    created_at: Timestamp,
    inputs: BTreeSet<ArtifactRef>,
    tool: Option<Tool>,
    epochs: EpochSet,
}

impl Provenance {
    /// The three members both schemas require.
    ///
    /// `inputs` may legitimately be empty — a fresh proposal is derived from nothing — but
    /// the member is always present, which is why it is a collection rather than an option.
    #[must_use]
    pub fn new(
        actor: ActorId,
        created_at: Timestamp,
        inputs: impl IntoIterator<Item = ArtifactRef>,
    ) -> Self {
        Self {
            actor,
            created_at,
            inputs: inputs.into_iter().collect(),
            tool: None,
            epochs: EpochSet::unpinned(),
        }
    }

    /// Name the tool or model version (docs/44: "actor/tool/model version").
    #[must_use]
    pub fn with_tool(mut self, tool: Tool) -> Self {
        self.tool = Some(tool);
        self
    }

    /// Pin the epochs this production happened under (plan §4.6).
    #[must_use]
    pub fn under_epochs(mut self, epochs: EpochSet) -> Self {
        self.epochs = epochs;
        self
    }

    /// Who produced it.
    #[must_use]
    pub const fn actor(&self) -> &ActorId {
        &self.actor
    }

    /// When.
    #[must_use]
    pub const fn created_at(&self) -> &Timestamp {
        &self.created_at
    }

    /// From what inputs, in canonical ascending order.
    pub fn inputs(&self) -> impl ExactSizeIterator<Item = &ArtifactRef> {
        self.inputs.iter()
    }

    /// Whether an artifact is among this record's inputs.
    #[must_use]
    pub fn has_input(&self, input: &ArtifactRef) -> bool {
        self.inputs.contains(input)
    }

    /// With what tool, when one was named.
    #[must_use]
    pub const fn tool(&self) -> Option<&Tool> {
        self.tool.as_ref()
    }

    /// Under which epochs (plan §4.6). Carried, never rendered — see the module docs.
    #[must_use]
    pub const fn epochs(&self) -> &EpochSet {
        &self.epochs
    }

    /// The provenance object exactly as the two schemas close it.
    ///
    /// Four members, three of them always present and `tool` present exactly when one was
    /// named. Nothing else: `additionalProperties` is false, so a field written where the
    /// schema does not admit it is as invalid as one missing where it does.
    #[must_use]
    pub fn to_record(&self) -> Value {
        let mut fields = vec![
            (field("actor"), self.actor.to_value()),
            (field("created_at"), self.created_at.to_value()),
            (
                field("inputs"),
                Value::seq(self.inputs.iter().map(ArtifactRef::to_value))
                    .expect("an input list nests two levels"),
            ),
        ];
        if let Some(tool) = &self.tool {
            fields.push((field("tool"), Value::text(tool.as_str())));
        }
        Value::record(fields).expect("the four member names are distinct compile-time constants")
    }

    /// The canonical preimage of this record, for a content identity.
    ///
    /// This is [`to_record`](Self::to_record) *plus* the epochs, because two productions
    /// that differ only in the epochs they were pinned under are two productions
    /// (plan §4.6: "an epoch advance never silently revalidates or invalidates a published
    /// receipt" — which requires the pinned epoch to be part of what was published).
    #[must_use]
    pub fn to_preimage(&self) -> Value {
        Value::record([
            (field("record"), self.to_record()),
            (field("epochs"), self.epochs_value()),
        ])
        .expect("two distinct compile-time constants")
    }

    /// The six epochs, all named, in [`EpochKind::ALL`] order.
    ///
    /// Every kind appears; an unpinned one renders `Null` rather than being dropped, which
    /// is the same discipline `EpochSet::entries` enforces on the result envelope — a record
    /// cannot quietly report five epochs.
    fn epochs_value(&self) -> Value {
        let entries = self.epochs.entries();
        Value::record(entries.iter().map(|entry| {
            let value = entry.binding.as_str().map_or(Value::Null, Value::text);
            (field_of(entry.kind), value)
        }))
        .expect("the six epoch kinds are distinct")
    }
}

/// The canonical field name of an epoch kind.
fn field_of(kind: EpochKind) -> Name {
    match kind {
        EpochKind::Protocol => field("protocol"),
        EpochKind::Semantic => field("semantic"),
        EpochKind::Intent => field("intent"),
        EpochKind::Evidence => field("evidence"),
        EpochKind::Proof => field("proof"),
        EpochKind::Corpus => field("corpus"),
    }
}

#[cfg(test)]
mod tests {
    use continuum_value::epoch::{EvidenceEpoch, ProtocolEpoch};

    use super::*;

    fn actor() -> ActorId {
        ActorId::new("agent:invariant-synthesizer").expect("well formed")
    }

    fn at() -> Timestamp {
        Timestamp::new("2026-08-01T12:00:00.000Z").expect("well formed")
    }

    fn artifact(text: &str) -> ArtifactRef {
        ArtifactRef::new(text).expect("well formed")
    }

    fn members(value: &Value) -> Vec<String> {
        match value {
            Value::Record(fields) => fields.keys().map(|name| name.as_str().to_owned()).collect(),
            other => panic!("expected a record, got {other:?}"),
        }
    }

    #[test]
    fn record_is_exactly_the_schema_object() {
        let provenance = Provenance::new(actor(), at(), [artifact("trace_9f")])
            .with_tool(Tool::new("continuum-observer/0.0.0").expect("non-empty"));
        // Four members and no fifth. The order is `Name`'s shortlex, which is what makes
        // the encoding canonical; the schema states no field order and has none to state.
        assert_eq!(
            members(&provenance.to_record()),
            ["tool", "actor", "inputs", "created_at"]
        );
    }

    #[test]
    fn a_tool_less_record_omits_the_member() {
        // `tool` is the schemas' one optional member; `additionalProperties: false` makes
        // rendering it empty as wrong as rendering an unknown field.
        let provenance = Provenance::new(actor(), at(), []);
        assert_eq!(
            members(&provenance.to_record()),
            ["actor", "inputs", "created_at"]
        );
        assert_eq!(provenance.tool(), None);
    }

    #[test]
    fn the_three_required_members_have_no_absent_spelling() {
        // Structural, not validated: `Provenance::new` takes all three and there is no
        // constructor that omits one.
        let provenance = Provenance::new(actor(), at(), []);
        assert_eq!(provenance.actor().as_str(), "agent:invariant-synthesizer");
        assert_eq!(provenance.created_at().as_str(), "2026-08-01T12:00:00.000Z");
        assert_eq!(provenance.inputs().len(), 0);
    }

    #[test]
    fn input_handles_match_the_artifact_pattern() {
        assert_eq!(ArtifactRef::PATTERN, "^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$");
        for text in [
            "trace_9f",
            "ev_1a-2b",
            "crash_c0_d1",
            "proof_receipt_x",
            "a_b",
        ] {
            assert_eq!(ArtifactRef::new(text).expect("well formed").as_str(), text);
        }
        assert_eq!(
            ArtifactRef::new("trace"),
            Err(ArtifactRefError::NoSeparator)
        );
        assert_eq!(ArtifactRef::new("_9f"), Err(ArtifactRefError::EmptyClass));
        assert_eq!(
            ArtifactRef::new("Trace_9f"),
            Err(ArtifactRefError::ClassCharacter)
        );
        assert_eq!(
            ArtifactRef::new("tr-ace_9f"),
            Err(ArtifactRefError::ClassCharacter)
        );
        assert_eq!(
            ArtifactRef::new("trace_"),
            Err(ArtifactRefError::EmptyIdentity)
        );
        assert_eq!(
            ArtifactRef::new("trace_9f/x"),
            Err(ArtifactRefError::IdentityCharacter)
        );
    }

    #[test]
    fn timestamps_are_the_wire_spelling() {
        assert_eq!(Timestamp::SHAPE, "YYYY-MM-DDTHH:MM:SS.sssZ");
        assert!(Timestamp::new("2026-08-01T12:00:00.000Z").is_ok());
        // A leap second is a real second.
        assert!(Timestamp::new("2026-06-30T23:59:60.000Z").is_ok());
        for text in [
            "2026-08-01T12:00:00Z",
            "2026-08-01 12:00:00.000Z",
            "2026-08-01T12:00:00.000+01:00",
            "",
        ] {
            assert_eq!(Timestamp::new(text), Err(TimestampError::NotCanonical));
        }
        for text in [
            "2026-13-01T12:00:00.000Z",
            "2026-08-32T12:00:00.000Z",
            "2026-08-01T24:00:00.000Z",
            "2026-08-01T12:60:00.000Z",
            "2026-08-01T12:00:61.000Z",
        ] {
            assert_eq!(Timestamp::new(text), Err(TimestampError::OutOfRange));
        }
    }

    #[test]
    fn nothing_here_reads_a_clock() {
        // GOV-1-04 checks this mechanically across the workspace; the assertion here is
        // that the type has no clock-reading constructor at all — `new` is the only one,
        // and it takes the text.
        let explicit = Timestamp::new("2026-08-01T12:00:00.000Z").expect("well formed");
        assert_eq!(explicit.to_string(), "2026-08-01T12:00:00.000Z");
    }

    #[test]
    fn inputs_are_a_set() {
        let repeated = Provenance::new(
            actor(),
            at(),
            [
                artifact("trace_b"),
                artifact("trace_a"),
                artifact("trace_b"),
            ],
        );
        let listed: Vec<&str> = repeated.inputs().map(ArtifactRef::as_str).collect();
        assert_eq!(listed, ["trace_a", "trace_b"]);
        assert!(repeated.has_input(&artifact("trace_a")));
        assert!(!repeated.has_input(&artifact("trace_z")));
    }

    #[test]
    fn provenance_value_is_order_independent() {
        // One derivation has one spelling, so it has one content identity (ADR-0013).
        let forward = Provenance::new(actor(), at(), [artifact("trace_a"), artifact("trace_b")]);
        let backward = Provenance::new(actor(), at(), [artifact("trace_b"), artifact("trace_a")]);
        assert_eq!(forward, backward);
        assert_eq!(forward.to_record().encode(), backward.to_record().encode());
        assert_eq!(
            forward.to_preimage().encode(),
            backward.to_preimage().encode()
        );
    }

    #[test]
    fn epochs_are_carried_and_never_rendered() {
        let pinned = Provenance::new(actor(), at(), []).under_epochs(
            EpochSet::unpinned()
                .with_protocol(ProtocolEpoch::new(1, 0))
                .with_evidence(EvidenceEpoch::new("ep_evidence_1").expect("well formed")),
        );
        // Carried: plan §4.6 makes evidence epoch-scoped.
        assert!(pinned.epochs().evidence().is_some());
        // Never rendered: the schema closes the provenance object at four members.
        assert_eq!(
            members(&pinned.to_record()),
            ["actor", "inputs", "created_at"]
        );
        // …and the epochs are part of what was produced, so they are in the preimage.
        let unpinned = Provenance::new(actor(), at(), []);
        assert_eq!(unpinned.to_record().encode(), pinned.to_record().encode());
        assert_ne!(
            unpinned.to_preimage().encode(),
            pinned.to_preimage().encode()
        );
    }

    #[test]
    fn the_preimage_names_all_six_epochs() {
        let provenance = Provenance::new(actor(), at(), []);
        let preimage = provenance.to_preimage();
        let Value::Record(fields) = &preimage else {
            panic!("expected a record");
        };
        let epochs = fields.get(&field("epochs")).expect("an epochs member");
        assert_eq!(
            members(epochs),
            // `Value::record` orders field names shortlex, which is why this is not
            // `EpochKind::ALL` order; all six are present, which is the claim.
            [
                "proof", "corpus", "intent", "evidence", "protocol", "semantic"
            ]
        );
        assert_eq!(EpochKind::ALL.len(), 6);
    }

    #[test]
    fn an_empty_tool_is_refused() {
        assert_eq!(Tool::new(""), Err(EmptyTool));
        assert_eq!(Tool::new("z3/4.13").expect("non-empty").as_str(), "z3/4.13");
        assert_eq!(EmptyTool.to_string(), "a tool name is not the empty string");
    }
}
