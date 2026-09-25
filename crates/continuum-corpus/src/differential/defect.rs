//! The engine-defect report a disagreement emits (plan §4.7).
//!
//! > Any `ReplayDiverged`, parity mismatch, engine crash, or explanation-validation
//! > failure emits a `defect_*` artifact: all inputs pinned by content identity,
//! > semantic and checker epochs, engine identity, and an automatically minimized
//! > reproduction.
//!
//! RFC 0026 correction 17 corrects "checker epochs" to the semantic and **proof**
//! epochs, and the RFC governs, so a report pins those two.
//!
//! **No schema governs this artifact.** `notes/plan/schemas/` has no `defect_*`
//! document (plan.review.5 A14 lists `defect_` among the handle classes with no schema
//! pattern), and the IDL declares only the handle (`DefectHandle = "defect_"`). This
//! module therefore defines no wire schema: [`DefectReport::encode`] is a canonical,
//! line-oriented text that is deterministic and hashes to the handle, and it is the
//! harness's report, not a protocol artifact. When a schema lands, the report is
//! projected onto it and this encoding is retired.
//!
//! What it pins:
//!
//! - both engines' slot and build;
//! - the claims the disagreement halted;
//! - the semantic and proof epochs, as given by the caller (the proof epoch may be
//!   typed absent: no proof lane is involved in an engine comparison);
//! - the declared budget;
//! - the original fixture by model content identity (BLAKE3 over
//!   [`continuum_model_core::identity`]'s canonical encoding);
//! - the minimized reproduction by the same identity **and** by its canonical encoding
//!   in hex, which is the reproduction itself, and the disagreement it reproduces.
//!
//! Redaction (plan §18.4): the fixtures are generated or corpus models; no report field
//! carries host paths, environment, time, or user data.

use std::fmt::Write as _;

use continuum_model_core::model::Model;
use continuum_value::identity::{Blake3Hasher, ContentHasher};

use super::engine::{Budget, EngineIdentity};
use super::minimize::Minimality;

/// The epochs a report is meaningful under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Epochs {
    /// The semantic epoch token.
    pub semantic: String,
    /// The proof epoch token, or `None` when no proof lane is involved.
    pub proof: Option<String>,
}

/// A model pinned by content identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    /// The corpus label (not part of the identity).
    pub label: String,
    /// BLAKE3 of the model's canonical identity encoding, as 64 lowercase hex.
    pub identity: String,
    /// Deletable declarations.
    pub size: usize,
}

impl Pinned {
    /// Pin `model`.
    #[must_use]
    pub fn of(label: &str, model: &Model, size: usize) -> Self {
        Self {
            label: label.to_owned(),
            identity: Blake3Hasher::hash(model.identity().as_bytes()).to_token(),
            size,
        }
    }
}

/// The minimized reproduction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reproduction {
    /// The minimized model, pinned.
    pub model: Pinned,
    /// Its canonical identity encoding, in lowercase hex: the reproduction itself.
    pub encoding: String,
    /// The disagreement it reproduces, as rendered.
    pub disagreement: String,
    /// What minimization established.
    pub minimality: Minimality,
    /// Subset tests minimization ran.
    pub tests: usize,
}

impl Reproduction {
    /// The reproduction for `model`.
    #[must_use]
    pub fn of(
        label: &str,
        model: &Model,
        size: usize,
        disagreement: String,
        minimality: Minimality,
        tests: usize,
    ) -> Self {
        let identity = model.identity();
        let mut encoding = String::with_capacity(identity.as_bytes().len().saturating_mul(2));
        for byte in identity.as_bytes() {
            let _ = write!(encoding, "{byte:02x}");
        }
        Self {
            model: Pinned::of(label, model, size),
            encoding,
            disagreement,
            minimality,
            tests,
        }
    }
}

/// One engine-defect report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefectReport {
    /// The engine under test.
    pub subject: EngineIdentity,
    /// The engine it was measured against.
    pub oracle: EngineIdentity,
    /// The claims halted.
    pub claims: Vec<String>,
    /// The epochs.
    pub epochs: Epochs,
    /// The declared budget.
    pub budget: Budget,
    /// The fixture the disagreement was found on.
    pub original: Pinned,
    /// The disagreement on the original, as rendered.
    pub disagreement: String,
    /// Every engine fault this report's evidence shows, as `(slot, rendered fault)`,
    /// sorted and without repeats: the retained disagreement when it is a fault, and
    /// every fault on the fixture it was found on (including a recheck). Encoded, so
    /// the handle is derived over them, and the only source the harness reads when it
    /// quarantines a slot under this report (cr-2r0m24 round 5).
    pub faults: Vec<(String, String)>,
    /// The minimized reproduction.
    pub reproduction: Reproduction,
}

/// The encoding's first line; bumps when the encoding changes.
pub const FORMAT: &str = "continuum-corpus-defect/2";

impl DefectReport {
    /// The canonical encoding: one `key value` line per field, fixed order, `\n`
    /// terminated. Deterministic: a function of the fields only.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut out = String::new();
        // Every value is escaped (`\\` and newline), so a value cannot forge a
        // following line and two different reports cannot share an encoding.
        let mut line = |key: &str, value: &str| {
            out.push_str(key);
            out.push(' ');
            for character in value.chars() {
                match character {
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    other => out.push(other),
                }
            }
            out.push('\n');
        };
        line("format", FORMAT);
        line("subject.slot", &self.subject.slot);
        line("subject.build", &self.subject.build);
        line("oracle.slot", &self.oracle.slot);
        line("oracle.build", &self.oracle.build);
        line("claims.count", &self.claims.len().to_string());
        for claim in &self.claims {
            line("claim", claim);
        }
        line("epoch.semantic", &self.epochs.semantic);
        match &self.epochs.proof {
            Some(proof) => line("epoch.proof.present", proof),
            None => line("epoch.proof.absent", "-"),
        }
        line(
            "budget",
            &format!(
                "states={} depth={} transitions={}",
                self.budget.states, self.budget.depth, self.budget.transitions
            ),
        );
        line("original.label", &self.original.label);
        line(
            "original.identity",
            &format!("blake3-256:{}", self.original.identity),
        );
        line("original.size", &self.original.size.to_string());
        line("original.disagreement", &self.disagreement);
        line("faults.count", &self.faults.len().to_string());
        for (slot, fault) in &self.faults {
            line("fault.slot", slot);
            line("fault.evidence", fault);
        }
        line(
            "minimized.identity",
            &format!("blake3-256:{}", self.reproduction.model.identity),
        );
        line("minimized.size", &self.reproduction.model.size.to_string());
        line(
            "minimized.minimality",
            self.reproduction.minimality.as_str(),
        );
        line("minimized.tests", &self.reproduction.tests.to_string());
        line("minimized.disagreement", &self.reproduction.disagreement);
        line("minimized.encoding", &self.reproduction.encoding);
        out
    }

    /// The `defect_*` handle: BLAKE3 of [`Self::encode`], as a plan §4.4 handle.
    #[must_use]
    pub fn handle(&self) -> String {
        format!(
            "defect_{}",
            Blake3Hasher::hash(self.encode().as_bytes()).to_token()
        )
    }

    /// The slots whose fault this report encodes.
    #[must_use]
    pub fn faulted_slots(&self) -> std::collections::BTreeSet<&str> {
        self.faults.iter().map(|(slot, _)| slot.as_str()).collect()
    }
}

/// Why an engine's contract could not be read at assembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContractFault {
    /// [`super::Engine::identity`] panicked.
    IdentityPanicked,
    /// [`super::Engine::fields`] panicked.
    FieldsPanicked,
    /// The engine names a slot other than the one it was plugged into.
    SlotMismatch,
    /// The engine declares fields other than a lane it serves requires.
    FieldsMismatch,
}

impl ContractFault {
    /// The fault's stable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IdentityPanicked => "identity-panicked",
            Self::FieldsPanicked => "fields-panicked",
            Self::SlotMismatch => "slot-mismatch",
            Self::FieldsMismatch => "fields-mismatch",
        }
    }
}

/// The defect report for an engine that faulted before evaluating anything: no model
/// is involved, so the report pins the slot it was plugged into (the configuration
/// names it, not the engine), the fault, the claims halted, and the epochs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractDefect {
    /// The slot the configuration plugged the engine into.
    pub slot: String,
    /// What failed.
    pub fault: ContractFault,
    /// The panic text or the mismatch, for the report.
    pub detail: String,
    /// The build the engine names, when its identity could be read.
    pub build: Option<String>,
    /// Every claim of every lane the slot serves.
    pub claims: Vec<String>,
    /// The epochs.
    pub epochs: Epochs,
}

/// The contract-defect encoding's first line.
pub const CONTRACT_FORMAT: &str = "continuum-corpus-contract-defect/1";

impl ContractDefect {
    /// Canonical encoding, escaped exactly as [`DefectReport::encode`] escapes.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut out = String::new();
        let mut line = |key: &str, value: &str| {
            out.push_str(key);
            out.push(' ');
            for character in value.chars() {
                match character {
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    other => out.push(other),
                }
            }
            out.push('\n');
        };
        line("format", CONTRACT_FORMAT);
        line("slot", &self.slot);
        line("fault", self.fault.as_str());
        line("detail", &self.detail);
        match &self.build {
            Some(build) => line("build.present", build),
            None => line("build.absent", "-"),
        }
        line("claims.count", &self.claims.len().to_string());
        for claim in &self.claims {
            line("claim", claim);
        }
        line("epoch.semantic", &self.epochs.semantic);
        match &self.epochs.proof {
            Some(proof) => line("epoch.proof.present", proof),
            None => line("epoch.proof.absent", "-"),
        }
        out
    }

    /// The `defect_*` handle: BLAKE3 of [`Self::encode`].
    #[must_use]
    pub fn handle(&self) -> String {
        format!(
            "defect_{}",
            Blake3Hasher::hash(self.encode().as_bytes()).to_token()
        )
    }
}
