//! Model actions: the `selected[].kind = "model"` half of PR-11 / IMPL-03 (RFC 0028,
//! "Required fields, reconciled with plan §6.2", the "relevant source spans and model
//! actions" row).
//!
//! # Scope: actions, not the whole model vocabulary
//!
//! Plan §6.2 and RFC 0028 both say "model **actions**", not "model elements" — PR 8's
//! `continuum-engine-reference` model core separately declares named variables, named
//! actions, and named predicates, and only the middle one is this bullet's word.
//! [`ModelActionRef`] is scoped to that word on purpose: broadening it to variables or
//! predicates would be vocabulary this bone was not asked to invent, and the schema's
//! `model` selection kind has no per-item sub-shape (`selected[]` items are uniformly
//! `{id, kind, summary, artifact}`) that would need one anyway.
//!
//! # The action name is typed, not a raw string
//!
//! `correspondence.bind`'s wire request types the semantic side as `element: String
//! required` (plan §16's source/semantic correspondence). This module strengthens that to
//! [`continuum_value::value::Name`] — "a canonical identifier: a symbol, a record field, a
//! variant tag, or an opaque domain" (`continuum-value`, already this workspace's shared
//! leaf for exactly this grammar) — non-empty printable ASCII, one canonical spelling.
//! `continuum-engine-reference` independently keeps its own `Ident` newtype over the same
//! grammar for a kernel-wire-order reason that does not apply here (see that module's own
//! "Why not `continuum_value::value::Name`"), so reusing `Name` directly, rather than
//! adding a third copy of the grammar, is the right call in this crate.
//!
//! # The model artifact, when there is one
//!
//! Plan §4.4 names `model_*` — an elaborated model — as the artifact class an action
//! belongs to (`continuum_workspace::artifact_path::ArtifactClass::ElaboratedModel`).
//! [`ModelActionRef::with_model`] accepts a real handle of exactly that class and refuses
//! any other; [`ModelActionRef::action_only`] carries none, which the schema reads as "the
//! item is derived and has no standalone artifact" rather than "unverifiable" (RFC 0028,
//! "Selection and the causal core"). **This crate cannot mint a `model_*` handle itself**:
//! `continuum-cml-elab` (elaboration) is an unopened PR-1 scaffold in this workspace, so
//! there is no elaborated-model artifact anywhere yet to reference. What is delivered is
//! the typed slot and the class check; producing a real handle to put in it is the
//! elaborator's bullet, not this one.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the action name is a canonical `Name`, not a bare string | this module's own reading, above | `an_empty_or_control_action_name_is_refused` (via `Name::new`) |
//! | the model artifact, when present, must be class `ElaboratedModel` | plan §4.4 | `with_model_refuses_the_wrong_artifact_class`, anti-vacuity `with_model_accepts_the_right_artifact_class` |
//! | kind is `model` | `context-pack.schema.json` | `a_model_action_ref_projects_to_the_model_kind` |
//! | `artifact` is the model handle when present, `null` otherwise | RFC 0028, "Selection and the causal core" | `artifact_reflects_the_model_handle`, `action_only_has_no_artifact` |
//! | the summary is a pure function of the ref's typed fields, never of caller text | INV-016 | `summary_names_the_action_and_model`, and the method signature itself (no string parameter) |

use core::fmt;

use continuum_value::value::Name;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};

use crate::selection::{SelectedItem, SelectionKind};

/// A typed reference to a model action: the `selected[].kind = "model"` selection kind.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModelActionRef {
    action: Name,
    model: Option<ArtifactHandle>,
}

impl ModelActionRef {
    /// Reference `action`, naming no elaborated-model artifact.
    #[must_use]
    pub const fn action_only(action: Name) -> Self {
        Self {
            action,
            model: None,
        }
    }

    /// Reference `action` inside the elaborated model named by `model`.
    ///
    /// # Errors
    ///
    /// [`ModelActionRefError::WrongArtifactClass`] when `model`'s class is not
    /// [`ArtifactClass::ElaboratedModel`] — plan §4.4's `model_*`.
    pub fn with_model(action: Name, model: ArtifactHandle) -> Result<Self, ModelActionRefError> {
        if model.class() != ArtifactClass::ElaboratedModel {
            return Err(ModelActionRefError::WrongArtifactClass(model.class()));
        }
        Ok(Self {
            action,
            model: Some(model),
        })
    }

    /// The referenced action's name.
    #[must_use]
    pub const fn action(&self) -> &Name {
        &self.action
    }

    /// The elaborated model this action belongs to, when known.
    #[must_use]
    pub const fn model(&self) -> Option<&ArtifactHandle> {
        self.model.as_ref()
    }

    /// Project this reference into the pack's generic `selected[]` shape, under `id`.
    ///
    /// `artifact` is `self.model` (already class-checked at construction) and `summary`
    /// is built only from `self.action` and `self.model` — no string parameter through
    /// which caller-supplied text could enter (INV-016).
    #[must_use]
    pub fn into_selected_item(self, id: Name) -> SelectedItem {
        let summary = match &self.model {
            Some(model) => format!("model action `{}` in {model}", self.action),
            None => format!("model action `{}`", self.action),
        };
        SelectedItem::new(id, SelectionKind::Model, summary, self.model)
    }
}

/// Why a [`ModelActionRef`] was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelActionRefError {
    /// The supplied handle's class was not [`ArtifactClass::ElaboratedModel`].
    WrongArtifactClass(ArtifactClass),
}

impl fmt::Display for ModelActionRefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongArtifactClass(class) => write!(
                f,
                "a model action's artifact must be class `{}` (plan §4.4 `model_*`), not `{class}`",
                ArtifactClass::ElaboratedModel
            ),
        }
    }
}

impl core::error::Error for ModelActionRefError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(text: &str) -> Name {
        Name::new(text).expect("well-formed test action name")
    }

    fn id(text: &str) -> Name {
        Name::new(text).expect("well-formed test id")
    }

    fn model_handle(identity: &str) -> ArtifactHandle {
        ArtifactHandle::new(ArtifactClass::ElaboratedModel, identity).expect("well-formed handle")
    }

    #[test]
    fn an_empty_or_control_action_name_is_refused() {
        // `Name::new` is this module's own vocabulary for "typed, not stringly"; pin that
        // an action name goes through it and is not accepted unchecked.
        assert!(Name::new("").is_err());
        assert!(Name::new("has\nnewline").is_err());
        assert!(Name::new("FillBig").is_ok());
    }

    #[test]
    fn with_model_accepts_the_right_artifact_class() {
        let handle = model_handle("diehard1");
        let reference = ModelActionRef::with_model(action("FillBig"), handle.clone())
            .expect("elaborated-model class is accepted");
        assert_eq!(reference.model(), Some(&handle));
    }

    #[test]
    fn with_model_refuses_the_wrong_artifact_class() {
        // Anti-vacuity companion to the accept case above: the class check must actually
        // discriminate, not accept (or refuse) every class.
        let wrong = ArtifactHandle::new(ArtifactClass::Crashpack, "cp1").expect("well-formed");
        assert_eq!(
            ModelActionRef::with_model(action("FillBig"), wrong),
            Err(ModelActionRefError::WrongArtifactClass(
                ArtifactClass::Crashpack
            ))
        );
    }

    #[test]
    fn a_model_action_ref_projects_to_the_model_kind() {
        let item = ModelActionRef::action_only(action("FillBig")).into_selected_item(id("e_act"));
        assert_eq!(item.kind(), SelectionKind::Model);
        assert_eq!(item.id().as_str(), "e_act");
    }

    #[test]
    fn artifact_reflects_the_model_handle() {
        let handle = model_handle("diehard1");
        let item = ModelActionRef::with_model(action("FillBig"), handle.clone())
            .expect("accepted")
            .into_selected_item(id("e_act"));
        assert_eq!(item.artifact(), Some(&handle));
        assert_eq!(
            item.to_json().as_object().unwrap()["artifact"].as_str(),
            Some("model_diehard1")
        );
    }

    #[test]
    fn action_only_has_no_artifact() {
        let item = ModelActionRef::action_only(action("FillBig")).into_selected_item(id("e_act"));
        assert_eq!(item.artifact(), None);
        assert!(item.to_json().as_object().unwrap()["artifact"].is_null());
    }

    #[test]
    fn summary_names_the_action_and_model() {
        let without =
            ModelActionRef::action_only(action("FillBig")).into_selected_item(id("e_act"));
        assert_eq!(without.summary(), "model action `FillBig`");

        let handle = model_handle("diehard1");
        let with = ModelActionRef::with_model(action("FillBig"), handle)
            .expect("accepted")
            .into_selected_item(id("e_act"));
        assert_eq!(with.summary(), "model action `FillBig` in model_diehard1");
    }
}
