//! The pack's `replay` field: PR-11 / IMPL-05 (RFC 0028, "Required fields, reconciled
//! with plan §6.2", the "exact replay and debugger handles" row).
//!
//! # Scope: one nullable handle, not a `selected[].kind` member
//!
//! > exact replay and debugger handles | `replay`, `debugger_branch` | `^crash_`
//! > nullable; `^dbg_` nullable | `replay` MUST be non-null whenever `ReplayPreserving`
//! > is claimed
//! >
//! > — RFC 0028, "Required fields, reconciled with plan §6.2"
//!
//! `replay` is a **top-level pack field**, required by the schema and nullable, not one
//! of `selected[]`'s eleven items:
//!
//! ```text
//! "replay": {
//!   "anyOf": [
//!     { "pattern": "^crash_[A-Za-z0-9_-]+$", "type": "string" },
//!     { "type": "null" }
//!   ]
//! }
//! ```
//!
//! — `notes/plan/schemas/context-pack.schema.json`, `properties.replay`
//!
//! This bullet's own bookkeeping says the same thing twice: `crates/continuum-context/src/lib.rs`
//! lists "target/verdict/assurance and the replay reference (IMPL-01, IMPL-05)" as fields an
//! expansion child *inherits*, not as `selected[]` content, and RFC 0028 spells the eleven
//! `selected[].kind` members exhaustively — "`event`, `state_delta`, `source`, `model`,
//! `proof`, `assumption`, `counterfactual`, `obligation_flow`, `order_constraint`,
//! `repair_surface`, `unknown`" ("Selection and the causal core") — with no `replay` among
//! them. So [`ReplayRef`] does not project into [`crate::selection::SelectedItem`] the way
//! [`crate::source::SourceRef`] and [`crate::model::ModelActionRef`] do: there is no
//! `SelectionKind::Replay` to hold, and inventing one would be exactly the closed-enum
//! breaking change "adding … a member is a breaking change requiring a `schema_epoch`
//! advance" (RFC 0028, "Versioning and revision") forbids without one.
//!
//! # What the payload honestly is
//!
//! The schema names one shape for `replay` and no other: a `crash_*` [`ArtifactHandle`] —
//! plan §4.4's `crash_* crashpack` class — or `null`. Nothing in `context-pack.schema.json`
//! or RFC 0028 gives the *pack's* `replay` field a second field, an inline command, or a
//! trace. [`ReplayRef`] carries exactly that one handle, class-checked at construction on
//! the [`crate::model::ModelActionRef::with_model`] pattern, and nothing else — per this
//! bullet's own instruction not to guess a shape the texts do not name.
//!
//! It would be easy to over-read the payload from two adjacent texts, and both are worth
//! naming so the refusal to follow them is not silent:
//!
//! - `continuum-engine-reference`'s witness module produces "a sequence of `(action,
//!   state)` pairs starting at a declared initial state" (`crates/continuum-engine-reference/src/witness.rs`)
//!   — the engine-grain shape a replay *is*, conceptually. But that sequence is the
//!   *content* of a crashpack, not of the pack's `replay` field, which names the crashpack
//!   rather than embedding it — the same handle-vs-content split
//!   [`crate::model::ModelActionRef`]'s module documentation draws for `model_*`.
//! - `notes/plan/schemas/crashpack.schema.json` itself declares a field it also spells
//!   `replay` — `{command: [string, …], working_directory, environment, choice_log,
//!   expected_verdict}`, a shell reproduction recipe — one level down, inside the artifact
//!   `crash_*` names. That is a different schema's field reusing this bullet's word by
//!   coincidence, not a second shape for *this* one; `context-pack.schema.json`'s `replay`
//!   is the bare pattern above and nothing a crashpack's internals add can widen it.
//!
//! # Why this crate cannot mint the handle it references
//!
//! > the depth-**6** shortest witness … does have a wire home and always did:
//! > `VerificationResult.crashpack`, whose artifact class is `schemas/crashpack.schema.json`.
//! > What is missing is a producer — nothing in this workspace builds a crashpack
//! >
//! > — `crates/continuumd/src/daemon/task.rs`
//!
//! The same declined-scope shape [`crate::model::ModelActionRef::with_model`] states for
//! `model_*` applies here, stated once more rather than left implicit: [`ReplayRef::new`]
//! accepts an already-minted `crash_*` handle from a caller and class-checks it; it does not
//! — and, absent an in-tree crashpack producer, cannot — build one from a witness, a
//! verdict, or anything else.
//!
//! # What is declined
//!
//! - **The cross-field rule that `replay` MUST be non-null.** RFC 0028 ties non-null
//!   `replay` to two separate conditions — "a pack claiming `ReplayPreserving` MUST also
//!   claim and check `CausallyClosed`, and MUST carry a non-null `replay`" (C2), and "a
//!   question about a `refuted` or `deadlock` verdict … `replay` MUST be non-null" (the
//!   failure pack profile) — both statements about a *whole pack*'s other fields
//!   (`guarantees`, `verdict`), which this module does not have. Checking them is pack
//!   assembly's job, "outside this bullet's fence" on the same reasoning
//!   `crate::selection`'s module documentation gives for declining per-item content
//!   identity.
//! - **Running the replay.** "Replay preservation … replay execution against `replay`
//!   (INV-006)" (RFC 0028, guarantee-checker table) is a checker that loads the named
//!   crashpack and re-runs the model to compare verdicts — `continuum-engine-reference`'s
//!   job, not a reference type's. [`ReplayRef`] names the crashpack; it does not open it.
//! - **`debugger_branch` (`dbg_*`).** The same required-fields row that names `replay`
//!   names `debugger_branch` beside it, but no PR-11 bullet claims that word — RFC 0029's
//!   debugger, not this one — so this module builds no typed reference for it.

use core::fmt;

use continuum_intent::canonical_json::Json;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};

/// A typed reference to a crashpack: the pack's `replay` field when non-null
/// (`context-pack.schema.json` `properties.replay`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReplayRef {
    crashpack: ArtifactHandle,
}

impl ReplayRef {
    /// Reference `crashpack` as the pack's `replay` value.
    ///
    /// # Errors
    ///
    /// [`ReplayRefError::WrongArtifactClass`] when `crashpack`'s class is not
    /// [`ArtifactClass::Crashpack`] — plan §4.4's `crash_*`.
    pub fn new(crashpack: ArtifactHandle) -> Result<Self, ReplayRefError> {
        if crashpack.class() != ArtifactClass::Crashpack {
            return Err(ReplayRefError::WrongArtifactClass(crashpack.class()));
        }
        Ok(Self { crashpack })
    }

    /// The referenced crashpack.
    #[must_use]
    pub const fn crashpack(&self) -> &ArtifactHandle {
        &self.crashpack
    }

    /// The canonical JSON rendering of this reference: the handle's wire string alone,
    /// matching `context-pack.schema.json`'s `^crash_[A-Za-z0-9_-]+$` pattern for a
    /// non-null `replay`. The `null` case belongs to whatever assembles the whole pack
    /// (see the module documentation's "What is declined"), not to this type, which by
    /// construction always names a handle.
    #[must_use]
    pub fn to_json(&self) -> Json {
        Json::String(self.crashpack.to_string())
    }

    /// The canonical JSON bytes of [`to_json`](Self::to_json).
    #[must_use]
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        self.to_json().to_canonical_bytes()
    }
}

/// Why a [`ReplayRef`] was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayRefError {
    /// The supplied handle's class was not [`ArtifactClass::Crashpack`].
    WrongArtifactClass(ArtifactClass),
}

impl fmt::Display for ReplayRefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongArtifactClass(class) => write!(
                f,
                "a replay reference's artifact must be class `{}` (plan §4.4 `crash_`), not `{class}`",
                ArtifactClass::Crashpack
            ),
        }
    }
}

impl core::error::Error for ReplayRefError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection::SelectionKind;

    fn crashpack(identity: &str) -> ArtifactHandle {
        ArtifactHandle::new(ArtifactClass::Crashpack, identity).expect("well-formed handle")
    }

    #[test]
    fn new_accepts_a_crashpack_handle() {
        let handle = crashpack("cp7m3x9");
        let reference = ReplayRef::new(handle.clone()).expect("crashpack class is accepted");
        assert_eq!(reference.crashpack(), &handle);
    }

    #[test]
    fn new_refuses_the_wrong_artifact_class() {
        // Anti-vacuity companion to the accept case above: the class check must actually
        // discriminate, not accept (or refuse) every class.
        let wrong = ArtifactHandle::new(ArtifactClass::ElaboratedModel, "d1").expect("well-formed");
        assert_eq!(
            ReplayRef::new(wrong),
            Err(ReplayRefError::WrongArtifactClass(
                ArtifactClass::ElaboratedModel
            ))
        );
    }

    #[test]
    fn to_json_is_exactly_the_handles_wire_string() {
        let reference = ReplayRef::new(crashpack("cp7m3x9")).expect("accepted");
        assert_eq!(
            reference.to_json(),
            Json::String("crash_cp7m3x9".to_owned())
        );
        assert_eq!(reference.to_canonical_bytes(), br#""crash_cp7m3x9""#);
    }

    #[test]
    fn equal_handles_produce_byte_identical_references() {
        let build = || ReplayRef::new(crashpack("cp7m3x9")).expect("accepted");
        assert_eq!(build(), build());
        assert_eq!(build().to_canonical_bytes(), build().to_canonical_bytes());
    }

    #[test]
    fn distinct_handles_produce_distinct_references() {
        // Anti-vacuity companion to the identity test above: the encoding is not a
        // constant.
        let a = ReplayRef::new(crashpack("cp7m3x9")).expect("accepted");
        let b = ReplayRef::new(crashpack("cp7m3x8")).expect("accepted");
        assert_ne!(a, b);
        assert_ne!(a.to_canonical_bytes(), b.to_canonical_bytes());
    }

    #[test]
    fn replay_is_not_a_selection_kind() {
        // Regression pin for the module documentation's central claim: `replay` is a
        // top-level pack field, not one of the eleven `selected[].kind` members, and
        // `"replay"` is not a wire token any `SelectionKind` recognizes.
        assert_eq!(SelectionKind::from_wire_str("replay"), None);
        assert!(
            !SelectionKind::ALL
                .iter()
                .any(|kind| kind.as_wire_str() == "replay")
        );
    }
}
