//! Events: the `selected[].kind = "event"` half of PR-11 / IMPL-02 (RFC 0028,
//! "Required fields, reconciled with plan §6.2", the "causal core and surrounding
//! frontier" row; RFC 0001, the Causal Intermediate Representation).
//!
//! # Scope: which event, and which of the two causal-core roles docs/38 names
//!
//! > causal core and surrounding frontier | `selected[]` with `kind` in `event`,
//! > `order_constraint` | array | The core is the sub-slice required by the claimed
//! > preserved guarantees; the frontier is optional context
//! >
//! > — RFC 0028, "Required fields, reconciled with plan §6.2"
//!
//! `order_constraint` is a distinct kind and no part of this module's fence. What is
//! left for `event` is exactly the wording `crate::selection::SelectionKind::Event`
//! already committed to at IMPL-03 landing: "an event directly observed by the
//! property, or a causal predecessor." docs/38's own causal-core list names the same
//! pair first:
//!
//! > A causal core is closed under the dependencies needed to reproduce the
//! > violation. It may include: events directly observed by the property; causal
//! > predecessors; conflicts that selected the branch; obligation and resource
//! > transfers; fault preconditions; abstraction-relevant hidden events.
//! >
//! > — docs/38, "Causal core"
//!
//! Of the six bullets, two are `event`'s (the two above); RFC 0028 routes conflicts
//! and obligation/resource transfers to `obligation_flow`'s own row, not this one, and
//! neither RFC 0028's mapping table nor `SelectionKind`'s own doc comment names fault
//! preconditions or abstraction-relevant hidden events as `event` members — widening
//! [`EventRole`] to them would be inventing a reading neither governing text states.
//!
//! # The event identifier: `Name`, not a CIR `EventId`
//!
//! RFC 0001 gives events a semantic hash identity —
//!
//! ```text
//! EventId = H(
//!     cir_semantics_version, origin, actor_epoch, local_operation_index,
//!     normalized_label, normalized_inputs, canonical_causal_predecessor_ids
//! )
//! ```
//!
//! — and `schemas/cir.schema.json` fixes its wire spelling
//! (`^e:[A-Za-z0-9._:-]+$`). Neither is constructible here: `continuum-cir`, the crate
//! that owns "event and configuration validation" (that crate's own module
//! documentation, docs/01 §4.2, PR 17), is a scaffold with no `Event`/`EventId` type
//! at the time this bone lands, and this bone adds no dependency on it — there is
//! nothing built yet to depend on. Building RFC 0001's hash construction inside
//! `continuum-context` would invent `continuum-cir`'s own artifact ahead of the bone
//! that owns it, precisely the overreach `crate::model`'s `ModelActionRef::with_model`
//! already declined for `continuum-cml-elab`'s `model_*` handles, on the same
//! "unopened scaffold" ground (see that module's "The model artifact, when there is
//! one"). [`EventRef`] therefore names its event the way `ModelActionRef` names its
//! action: [`continuum_value::value::Name`], this workspace's shared
//! canonical-identifier grammar, already a dependency of this crate. A `Name` is not a
//! claim that the identifier is a real CIR `EventId`; it is the same declined-scope
//! honesty applied one layer earlier.
//!
//! # The artifact, when there is one
//!
//! Plan §4.4 names `cir_*` — a causal execution graph — as the artifact class RFC
//! 0001's own event set is drawn from ("A finite prime event-structure fragment is
//! represented by … where \(E\) is a finite set of event identities"; RFC 0001,
//! "Semantic object"). [`EventRef::with_execution_graph`] accepts a real handle of
//! exactly that class and refuses any other — the same discipline
//! `ModelActionRef::with_model` applies to `model_*`; [`EventRef::new`] carries none,
//! which the schema reads as "the item is derived and has no standalone artifact"
//! (RFC 0028, "Selection and the causal core").
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the event name is a canonical `Name`, not a bare string | this module's own reading, above | `an_empty_or_control_event_name_is_refused` (via `Name::new`) |
//! | the causal-execution-graph artifact, when present, must be class `CausalExecutionGraph` | plan §4.4 | `with_execution_graph_refuses_the_wrong_artifact_class`, anti-vacuity `with_execution_graph_accepts_the_right_artifact_class` |
//! | kind is `event` | `context-pack.schema.json` | `an_event_ref_projects_to_the_event_kind` |
//! | `artifact` is the graph handle when present, `null` otherwise | RFC 0028, "Selection and the causal core" | `artifact_reflects_the_execution_graph`, `new_has_no_artifact` |
//! | the two causal-core roles render distinctly and the summary is a pure function of the ref's typed fields, never of caller text | docs/38, "Causal core"; INV-016 | `summary_names_the_event_and_role`, and the method signature itself (no string parameter) |

use core::fmt;

use continuum_value::value::Name;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};

use crate::selection::{SelectedItem, SelectionKind};

/// Which of docs/38's two `event`-kind causal-core roles a reference plays.
///
/// > events directly observed by the property; causal predecessors; …
/// >
/// > — docs/38, "Causal core"
///
/// See the module documentation for why the other four causal-core bullets are not
/// members of this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventRole {
    /// An event directly observed by the property.
    Observed,
    /// A causal predecessor of an observed event.
    CausalPredecessor,
}

impl fmt::Display for EventRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Observed => "observed by the property",
            Self::CausalPredecessor => "a causal predecessor",
        })
    }
}

/// A typed reference to an event: the `selected[].kind = "event"` selection kind.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EventRef {
    event: Name,
    role: EventRole,
    execution_graph: Option<ArtifactHandle>,
}

impl EventRef {
    /// Reference `event` in the given `role`, naming no causal-execution-graph
    /// artifact.
    #[must_use]
    pub const fn new(event: Name, role: EventRole) -> Self {
        Self {
            event,
            role,
            execution_graph: None,
        }
    }

    /// Reference `event` in the given `role`, inside the causal execution graph named
    /// by `execution_graph`.
    ///
    /// # Errors
    ///
    /// [`EventRefError::WrongArtifactClass`] when `execution_graph`'s class is not
    /// [`ArtifactClass::CausalExecutionGraph`] — plan §4.4's `cir_*`.
    pub fn with_execution_graph(
        event: Name,
        role: EventRole,
        execution_graph: ArtifactHandle,
    ) -> Result<Self, EventRefError> {
        if execution_graph.class() != ArtifactClass::CausalExecutionGraph {
            return Err(EventRefError::WrongArtifactClass(execution_graph.class()));
        }
        Ok(Self {
            event,
            role,
            execution_graph: Some(execution_graph),
        })
    }

    /// The referenced event's name.
    #[must_use]
    pub const fn event(&self) -> &Name {
        &self.event
    }

    /// Which causal-core role this reference plays.
    #[must_use]
    pub const fn role(&self) -> EventRole {
        self.role
    }

    /// The causal execution graph this event belongs to, when known.
    #[must_use]
    pub const fn execution_graph(&self) -> Option<&ArtifactHandle> {
        self.execution_graph.as_ref()
    }

    /// Project this reference into the pack's generic `selected[]` shape, under `id`.
    ///
    /// `artifact` is `self.execution_graph` (already class-checked at construction)
    /// and `summary` is built only from `self.event` and `self.role` — no string
    /// parameter through which caller-supplied text could enter (INV-016).
    #[must_use]
    pub fn into_selected_item(self, id: Name) -> SelectedItem {
        let summary = match &self.execution_graph {
            Some(graph) => format!("event `{}`, {} (in {graph})", self.event, self.role),
            None => format!("event `{}`, {}", self.event, self.role),
        };
        SelectedItem::new(id, SelectionKind::Event, summary, self.execution_graph)
    }
}

/// Why an [`EventRef`] was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventRefError {
    /// The supplied handle's class was not [`ArtifactClass::CausalExecutionGraph`].
    WrongArtifactClass(ArtifactClass),
}

impl fmt::Display for EventRefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongArtifactClass(class) => write!(
                f,
                "an event's causal-execution-graph artifact must be class `{}` (plan §4.4 `cir_*`), not `{class}`",
                ArtifactClass::CausalExecutionGraph
            ),
        }
    }
}

impl core::error::Error for EventRefError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(text: &str) -> Name {
        Name::new(text).expect("well-formed test event name")
    }

    fn id(text: &str) -> Name {
        Name::new(text).expect("well-formed test id")
    }

    fn graph_handle(identity: &str) -> ArtifactHandle {
        ArtifactHandle::new(ArtifactClass::CausalExecutionGraph, identity)
            .expect("well-formed handle")
    }

    #[test]
    fn an_empty_or_control_event_name_is_refused() {
        // `Name::new` is this module's own vocabulary for "typed, not stringly"; pin
        // that an event name goes through it and is not accepted unchecked.
        assert!(Name::new("").is_err());
        assert!(Name::new("has\nnewline").is_err());
        assert!(Name::new("e7").is_ok());
    }

    #[test]
    fn with_execution_graph_accepts_the_right_artifact_class() {
        let handle = graph_handle("run1");
        let reference =
            EventRef::with_execution_graph(event("e7"), EventRole::Observed, handle.clone())
                .expect("causal-execution-graph class is accepted");
        assert_eq!(reference.execution_graph(), Some(&handle));
    }

    #[test]
    fn with_execution_graph_refuses_the_wrong_artifact_class() {
        // Anti-vacuity companion to the accept case above: the class check must
        // actually discriminate, not accept (or refuse) every class.
        let wrong = ArtifactHandle::new(ArtifactClass::Crashpack, "cp1").expect("well-formed");
        assert_eq!(
            EventRef::with_execution_graph(event("e7"), EventRole::Observed, wrong),
            Err(EventRefError::WrongArtifactClass(ArtifactClass::Crashpack))
        );
    }

    #[test]
    fn an_event_ref_projects_to_the_event_kind() {
        let item = EventRef::new(event("e7"), EventRole::Observed).into_selected_item(id("e_x"));
        assert_eq!(item.kind(), SelectionKind::Event);
        assert_eq!(item.id().as_str(), "e_x");
    }

    #[test]
    fn artifact_reflects_the_execution_graph() {
        let handle = graph_handle("run1");
        let item = EventRef::with_execution_graph(
            event("e7"),
            EventRole::CausalPredecessor,
            handle.clone(),
        )
        .expect("accepted")
        .into_selected_item(id("e_x"));
        assert_eq!(item.artifact(), Some(&handle));
        assert_eq!(
            item.to_json().as_object().unwrap()["artifact"].as_str(),
            Some("cir_run1")
        );
    }

    #[test]
    fn new_has_no_artifact() {
        let item = EventRef::new(event("e7"), EventRole::Observed).into_selected_item(id("e_x"));
        assert_eq!(item.artifact(), None);
        assert!(item.to_json().as_object().unwrap()["artifact"].is_null());
    }

    #[test]
    fn summary_names_the_event_and_role() {
        let observed =
            EventRef::new(event("e7"), EventRole::Observed).into_selected_item(id("e_x"));
        assert_eq!(observed.summary(), "event `e7`, observed by the property");

        let predecessor =
            EventRef::new(event("e3"), EventRole::CausalPredecessor).into_selected_item(id("e_y"));
        assert_eq!(predecessor.summary(), "event `e3`, a causal predecessor");

        let handle = graph_handle("run1");
        let with_graph = EventRef::with_execution_graph(event("e7"), EventRole::Observed, handle)
            .expect("accepted")
            .into_selected_item(id("e_z"));
        assert_eq!(
            with_graph.summary(),
            "event `e7`, observed by the property (in cir_run1)"
        );
    }
}
