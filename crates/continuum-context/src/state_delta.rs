//! State deltas: the `selected[].kind = "state_delta"` half of PR-11 / IMPL-02 (RFC
//! 0028, "Required fields, reconciled with plan §6.2", the "concrete and abstract
//! state deltas" row; docs/38, "State delta").
//!
//! # Shape: a per-variable delta, never a whole state
//!
//! > concrete and abstract state deltas | `selected[].kind = state_delta` | array item
//! > | Deltas, never whole states (docs/38, "State delta")
//! >
//! > — RFC 0028, "Required fields, reconciled with plan §6.2"
//!
//! docs/38 shows the shape a rendering owes, one variable per line:
//!
//! ```text
//! Concrete:
//!   disk.pending[e7]   + value X
//!   replies[e7]        reserved → published
//!
//! Abstract:
//!   acknowledged[e7]   false → true
//!   durable[e7]        false
//! ```
//!
//! — docs/38, "State delta"
//!
//! [`StateDeltaRef`] is exactly that line: which variable
//! ([`continuum_value::value::Name`]), whether the reading is concrete or abstract
//! ([`StateDeltaClass`] — RFC 0028's own two words, "concrete and abstract"), the
//! value it held before (when the delta names one — docs/38's `durable[e7] false` and
//! `+ value X` rows show no prior value, which this module reads as "none recorded"
//! rather than guessing one), and the value it holds after. `[e7]` — which event the
//! delta is keyed to — is deliberately absent; see "What is declined" below.
//!
//! # The value domain: `Value`, not the reference engine's `i64`
//!
//! Two typed value domains already exist in this workspace, and this module has to
//! pick one. `continuum-engine-reference`'s own `domain` module explains why the
//! choice is not close:
//!
//! > The certificate this engine must eventually emit carries a state as a vector of
//! > `i64` … `continuum_value::value::Value`'s richer lattice — sets, records,
//! > bitvectors, opaque domains — is the right vocabulary for the CIR and the wrong
//! > one here: it has no total injection into `i64` … That crate is not a dependency
//! > of this one.
//! >
//! > — `crates/continuum-engine-reference/src/domain.rs`
//!
//! A state delta is CIR-level content — docs/38's pipeline computes it *from* the
//! normalized causal graph, after replay, before the source/model/proof mapping, CIR
//! throughout — not the finite reference engine's own certificate-bound state vector.
//! So this module reaches for the vocabulary that module names as CIR's own:
//! [`continuum_value::value::Value`], already this crate's dependency and already the
//! type `crate::selection`'s own module documentation names for exactly this content
//! ("that encoding is for the exact finite value domain: states, model values"). RFC
//! 0028 also gains no dependency on `continuum-engine-reference` by the choice: `Value`
//! covers every docs/38 row directly — `replies[e7] reserved → published` is a
//! `Value::Symbol` transition and `acknowledged[e7] false → true` is `Value::Bool` —
//! neither of which has a natural `i64` reading, exactly the case the engine's own
//! module doc predicts.
//!
//! # A delta is a change
//!
//! [`StateDeltaRef::new`] refuses `before == Some(after)`: a record that names no
//! change is not the thing RFC 0028 names ("deltas, never whole states"), and
//! `crate::source`'s `SourceSpan` sets the precedent for construction-time refusal of
//! a value this module's own reading rules out, rather than accepting it and hoping a
//! caller notices.
//!
//! # What is declined
//!
//! - **Which event a delta is keyed to.** docs/38 annotates every line with `[e7]`.
//!   No typed event identity is constructible from this bone alone — see
//!   `crate::event`'s own "The event identifier" section, the same "unopened
//!   scaffold" ground `crate::model::ModelActionRef` already stands on for `model_*`
//!   — and inventing a cross-item link the schema does not carry (`selected[]` items
//!   are independent `{id, kind, summary, artifact}` records with no sibling-reference
//!   field) would be exactly the shape this bone was told not to guess. A compiler
//!   assembling a pack is free to select an `event`-kind item and a `state_delta`-kind
//!   item under related `id`s; this module does not need, and does not claim, a field
//!   for that relationship.
//! - **A standalone artifact.** Plan §4.4's nineteen-member class list has no entry
//!   for an individual per-variable delta; the closest artifact-shaped object,
//!   `crashpack.schema.json`'s `artifacts.state_diff`, is a bare `string` with no
//!   `ArtifactClass` token of its own — a Revision-2-era field this RFC does not
//!   reconcile. [`StateDeltaRef::into_selected_item`] therefore always emits
//!   `artifact: null`, on the same "no plan §4.4 class" ground `crate::source::SourceRef`
//!   already states for a source span.
//! - **Rendering `Value`'s composite kinds.** `Value` admits sets, records, maps, and
//!   more; [`render_value`] is total over every kind, but only the scalar kinds
//!   docs/38's own examples exercise — `Bool`, `Nat`, `Int`, `Text`, `Symbol` — get a
//!   reading tailored to them. A composite value renders through `Debug`, which never
//!   panics but is not claimed to be pretty; no corpus fixture this bone exercises
//!   one.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | `class` is exactly RFC 0028's two words, concrete and abstract | RFC 0028, above | `state_delta_class_displays_the_rfcs_own_words` |
//! | a delta naming no change is refused | this module's own reading, above | `a_no_op_delta_is_refused`, anti-vacuity `a_real_change_is_accepted` |
//! | kind is `state_delta` | `context-pack.schema.json` | `a_state_delta_ref_projects_to_the_state_delta_kind` |
//! | `artifact` is always `null` | plan §4.4 (no per-delta class) | `a_state_delta_ref_never_carries_an_artifact` |
//! | the summary is a pure function of the ref's typed fields, never of caller text | INV-016 | `summary_names_the_variable_and_values`, and the method signature itself (no string parameter) |
//! | two distinct deltas project to two distinct items | `rule ordering.deterministic` | `distinct_deltas_project_to_distinct_items` |
//! | value rendering is total, including composite kinds | this module's own reading, above | `render_value_is_total_over_composite_kinds` |

use core::fmt;

use continuum_value::value::{Name, Value};

use crate::selection::{SelectedItem, SelectionKind};

/// RFC 0028's own two words for a state delta's reading: "concrete and abstract state
/// deltas" (Required fields, reconciled with plan §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StateDeltaClass {
    /// A concrete state delta: `disk.pending[e7]`, `replies[e7]` in docs/38's example.
    Concrete,
    /// An abstract state delta: `acknowledged[e7]`, `durable[e7]` in docs/38's example.
    Abstract,
}

impl fmt::Display for StateDeltaClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Concrete => "concrete",
            Self::Abstract => "abstract",
        })
    }
}

/// A typed reference to a state delta: the `selected[].kind = "state_delta"`
/// selection kind.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StateDeltaRef {
    variable: Name,
    class: StateDeltaClass,
    before: Option<Value>,
    after: Value,
}

impl StateDeltaRef {
    /// Reference the delta of `variable`, of the given `class`, from `before` (when
    /// one is recorded) to `after`.
    ///
    /// # Errors
    ///
    /// [`StateDeltaRefError::NoChange`] when `before` is `Some(after.clone())` — a
    /// record naming no change is not a delta (see the module documentation).
    pub fn new(
        variable: Name,
        class: StateDeltaClass,
        before: Option<Value>,
        after: Value,
    ) -> Result<Self, StateDeltaRefError> {
        if before.as_ref() == Some(&after) {
            return Err(StateDeltaRefError::NoChange);
        }
        Ok(Self {
            variable,
            class,
            before,
            after,
        })
    }

    /// The variable this delta names.
    #[must_use]
    pub const fn variable(&self) -> &Name {
        &self.variable
    }

    /// Whether this is a concrete or an abstract reading.
    #[must_use]
    pub const fn class(&self) -> StateDeltaClass {
        self.class
    }

    /// The value before the delta, when one is recorded.
    #[must_use]
    pub const fn before(&self) -> Option<&Value> {
        self.before.as_ref()
    }

    /// The value after the delta.
    #[must_use]
    pub const fn after(&self) -> &Value {
        &self.after
    }

    /// Project this reference into the pack's generic `selected[]` shape, under `id`.
    ///
    /// `artifact` is always `null` (see the module documentation) and `summary` is
    /// built only from `self.variable`, `self.class`, `self.before`, and `self.after`
    /// — no string parameter through which caller-supplied text could enter
    /// (INV-016).
    #[must_use]
    pub fn into_selected_item(self, id: Name) -> SelectedItem {
        let summary = match &self.before {
            Some(before) => format!(
                "{} state delta: `{}` {} \u{2192} {}",
                self.class,
                self.variable,
                render_value(before),
                render_value(&self.after)
            ),
            None => format!(
                "{} state delta: `{}` {}",
                self.class,
                self.variable,
                render_value(&self.after)
            ),
        };
        SelectedItem::new(id, SelectionKind::StateDelta, summary, None)
    }
}

/// Render a [`Value`] for a summary.
///
/// Total over every [`Value`] kind; the scalar kinds docs/38's own examples exercise
/// (`Bool`, `Nat`, `Int`, `Text`, `Symbol`) get a reading tailored to them, and every
/// composite kind falls back to `Debug` (see the module documentation's "What is
/// declined").
fn render_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(flag) => flag.to_string(),
        Value::Nat(magnitude) => magnitude.to_string(),
        Value::Int(number) => number.to_string(),
        Value::Text(text) => format!("{text:?}"),
        Value::Symbol(name) => name.to_string(),
        composite => format!("{composite:?}"),
    }
}

/// Why a [`StateDeltaRef`] was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateDeltaRefError {
    /// `before` and `after` named the same value: not a delta.
    NoChange,
}

impl fmt::Display for StateDeltaRefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoChange => {
                f.write_str("before and after name the same value; a delta must change")
            }
        }
    }
}

impl core::error::Error for StateDeltaRefError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn variable(text: &str) -> Name {
        Name::new(text).expect("well-formed test variable name")
    }

    fn id(text: &str) -> Name {
        Name::new(text).expect("well-formed test id")
    }

    fn symbol(text: &str) -> Value {
        Value::symbol(variable(text))
    }

    #[test]
    fn state_delta_class_displays_the_rfcs_own_words() {
        assert_eq!(StateDeltaClass::Concrete.to_string(), "concrete");
        assert_eq!(StateDeltaClass::Abstract.to_string(), "abstract");
    }

    #[test]
    fn a_no_op_delta_is_refused() {
        assert_eq!(
            StateDeltaRef::new(
                variable("durable"),
                StateDeltaClass::Abstract,
                Some(Value::Bool(false)),
                Value::Bool(false),
            ),
            Err(StateDeltaRefError::NoChange)
        );
    }

    #[test]
    fn a_real_change_is_accepted() {
        // Anti-vacuity companion to the refusal above: the check must actually
        // discriminate, not refuse every construction.
        assert!(
            StateDeltaRef::new(
                variable("acknowledged"),
                StateDeltaClass::Abstract,
                Some(Value::Bool(false)),
                Value::Bool(true),
            )
            .is_ok()
        );
        assert!(
            StateDeltaRef::new(
                variable("durable"),
                StateDeltaClass::Abstract,
                None,
                Value::Bool(false),
            )
            .is_ok()
        );
    }

    #[test]
    fn a_state_delta_ref_projects_to_the_state_delta_kind() {
        let item = StateDeltaRef::new(
            variable("acknowledged"),
            StateDeltaClass::Abstract,
            Some(Value::Bool(false)),
            Value::Bool(true),
        )
        .expect("a real change")
        .into_selected_item(id("d_ack"));
        assert_eq!(item.kind(), SelectionKind::StateDelta);
        assert_eq!(item.id().as_str(), "d_ack");
    }

    #[test]
    fn a_state_delta_ref_never_carries_an_artifact() {
        let item = StateDeltaRef::new(
            variable("acknowledged"),
            StateDeltaClass::Abstract,
            Some(Value::Bool(false)),
            Value::Bool(true),
        )
        .expect("a real change")
        .into_selected_item(id("d_ack"));
        assert_eq!(item.artifact(), None);
        assert!(item.to_json().as_object().unwrap()["artifact"].is_null());
    }

    #[test]
    fn summary_names_the_variable_and_values() {
        // docs/38's four example rows, each representable without guessing a shape.
        let acknowledged = StateDeltaRef::new(
            variable("acknowledged"),
            StateDeltaClass::Abstract,
            Some(Value::Bool(false)),
            Value::Bool(true),
        )
        .expect("a real change")
        .into_selected_item(id("d1"));
        assert_eq!(
            acknowledged.summary(),
            "abstract state delta: `acknowledged` false \u{2192} true"
        );

        let durable = StateDeltaRef::new(
            variable("durable"),
            StateDeltaClass::Abstract,
            None,
            Value::Bool(false),
        )
        .expect("no prior value recorded")
        .into_selected_item(id("d2"));
        assert_eq!(durable.summary(), "abstract state delta: `durable` false");

        let replies = StateDeltaRef::new(
            variable("replies"),
            StateDeltaClass::Concrete,
            Some(symbol("reserved")),
            symbol("published"),
        )
        .expect("a real change")
        .into_selected_item(id("d3"));
        assert_eq!(
            replies.summary(),
            "concrete state delta: `replies` reserved \u{2192} published"
        );

        let disk_pending = StateDeltaRef::new(
            variable("disk.pending"),
            StateDeltaClass::Concrete,
            None,
            Value::text("X"),
        )
        .expect("no prior value recorded")
        .into_selected_item(id("d4"));
        assert_eq!(
            disk_pending.summary(),
            "concrete state delta: `disk.pending` \"X\""
        );
    }

    #[test]
    fn distinct_deltas_project_to_distinct_items() {
        let a = StateDeltaRef::new(
            variable("acknowledged"),
            StateDeltaClass::Abstract,
            Some(Value::Bool(false)),
            Value::Bool(true),
        )
        .expect("a real change")
        .into_selected_item(id("x"));
        let b = StateDeltaRef::new(
            variable("acknowledged"),
            StateDeltaClass::Abstract,
            Some(Value::Bool(false)),
            Value::Bool(false),
        );
        // `b` above is a no-op and is refused; build a genuinely distinct delta
        // instead (different variable) to compare against `a`.
        assert!(b.is_err());
        let c = StateDeltaRef::new(
            variable("durable"),
            StateDeltaClass::Abstract,
            Some(Value::Bool(false)),
            Value::Bool(true),
        )
        .expect("a real change")
        .into_selected_item(id("x"));
        assert_ne!(a, c);
        assert_ne!(a.to_canonical_bytes(), c.to_canonical_bytes());
    }

    #[test]
    fn render_value_is_total_over_composite_kinds() {
        let set = Value::set([Value::nat(1), Value::nat(2)]).expect("well-formed set");
        // Total: does not panic, and is not the empty string.
        assert!(!render_value(&set).is_empty());
        assert!(!render_value(&Value::Null).is_empty());
    }
}
