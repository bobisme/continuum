//! INV-014 receipts: what this checker established, and what it was told (PR 9).
//!
//! > Lean version, library closure, theorem hashes, axioms, certificate schema, and
//! > checker identity are recorded.
//! >
//! > — `notes/plan/plan.md`, INV-014 "Version-explicit proof"
//!
//! The receipt shape is fixed by `notes/plan/schemas/proof-receipt.schema.json`
//! (RFC 0024). [`Receipt::to_json`] emits an instance of that schema, and
//! `crates/continuum-kernel-core/tests/receipt.golden.json` is the byte-for-byte
//! output for the Die Hard closure fixture; `tools/check_kernel_covenant.py`
//! validates that same file against the schema, so a drift on either side fails a
//! gate.
//!
//! # Which epoch this receipt means
//!
//! Each of the four `continuum-kernel-*` crates declares its own `WIRE_EPOCH`, and
//! all four currently read `1`. They are *four different contracts* that happen to
//! share an ordinal: `CONTCERT 1`, `CONTSATC 1`, `CONTSMTC 1`, `CONTTMPC 1`. A
//! receipt that recorded a bare `1` would name none of them.
//!
//! So this crate's wire epoch is written only in its qualified spelling —
//! [`WIRE_EPOCH_ID`], `continuum-kernel-core/CONTCERT/1` — and it is carried in
//! `checker.version`, never in the receipt's `epochs` object:
//!
//! > **There is no checker epoch.** `EpochSet` has no `checker` member. […] checker
//! > identity is the `proof` epoch for Lean-backed checkers and engine identity for
//! > native checkers.
//! >
//! > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, correction 17
//!
//! `epochs` therefore carries the *artifact's* epochs — `semantic` read out of the
//! certificate envelope, `proof` supplied at the seam — and the checker's own
//! identity lives in the `checker` object where the schema puts it. START_HERE PR 9
//! and this bone's acceptance criteria say "checker epoch"; correction 17 calls that
//! phrasing an alias, and this module resolves the alias to checker *identity*.
//!
//! # The seam, and why it is not a digest this crate computes
//!
//! [`Seam`] is the receipt's trusted-input inventory: every field of a proof receipt
//! that a self-contained certificate checker structurally cannot derive. It is
//! modelled the way `CheckedClaim::trusted_components` already models the envelope
//! digests — carried, labelled, and named as trusted, never invented:
//!
//! - **Build digest and toolchain** (`checker.source_hash`, `checker.binary_hash`,
//!   `checker.toolchain`). A program cannot hash its own binary without reading it,
//!   and the workspace `Cargo.toml` records that bit-identical kernel builds are not
//!   yet claimed (`trim-paths` is unstable in the pinned toolchain). The build system
//!   knows these; the checker does not, and a self-reported digest that the build did
//!   not produce would be exactly the unearned claim INV-014 exists to prevent.
//! - **Input certificate hash** (`certificate.hash`). This crate takes `&[u8]` and
//!   computes no digest: a hash function in the trusted base is trusted code, and
//!   ADR-0013 makes hash equality *not* identity in certified lanes anyway. The
//!   producer's content-addressing layer owns the digest; the receipt records which
//!   digest it was handed.
//! - **`closure.observers_hash`** and **`epochs.proof`**. Neither appears anywhere in
//!   the wire form, so no certificate can supply them.
//! - **`receipt_id`** and **`reproduce`**. Minting an identity needs a clock or a
//!   counter, and the checker reads neither (INV-005 determinism); a reproduction
//!   command names paths the checker never sees.
//!
//! What the checker *does* refuse is a malformed seam: every field is shape-checked
//! as printable ASCII and `receipt_id` against the schema's pattern, so a caller
//! cannot smuggle a control character or an unspellable identity into a published
//! artifact. Shape, never contents — the same boundary the envelope digests get.
//!
//! # Only a verified verdict yields a receipt
//!
//! [`receipt`] returns [`ReceiptError::NotVerified`] for [`Verdict::Rejected`] and
//! [`Verdict::Unsupported`]. A receipt records a discharged claim; a rejection is a
//! statement about an artifact and an unsupported feature is not a verdict about the
//! model at all (INV-008). Neither has a `claim.verdict` the schema admits.

use crate::verdict::{CertificateKind, CheckedClaim, TokenFault, Verdict};
use crate::wire::{MAGIC, WIRE_EPOCH};

/// Artifact-class identity of the schema this receipt is written against.
pub const SCHEMA_ID: &str = "https://continuum.dev/schema/proof-receipt.json";

/// Schema epoch of `notes/plan/schemas/proof-receipt.schema.json`.
pub const SCHEMA_EPOCH: &str = "1";

/// This crate's name, as it appears in a receipt's `checker.name`.
pub const CHECKER_NAME: &str = "continuum-kernel-core";

/// The qualified spelling of this crate's wire epoch.
///
/// `<crate>/<magic>/<epoch>`. The crate name and the eight-byte magic are what
/// distinguish this `1` from the `1` of the three sibling kernel crates; see the
/// module documentation.
pub const WIRE_EPOCH_ID: &str = "continuum-kernel-core/CONTCERT/1";

/// The receipt fields a certificate checker cannot derive, and is therefore given.
///
/// Every field is a *trusted input*: the receipt records it, [`Receipt::trusted_inputs`]
/// names it, and this crate never invents a value for it. See the module
/// documentation for why each one is here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seam<'a> {
    /// `receipt_id` — must match the schema's `^receipt_[A-Za-z0-9_-]+$`.
    pub receipt_id: &'a str,
    /// `certificate.hash` — the content digest of the bytes that were checked.
    pub certificate_digest: &'a str,
    /// `closure.observers_hash` — the observer set the claim is relative to.
    pub observers_digest: &'a str,
    /// `epochs.proof` — the proof epoch in force, one of the six (RFC 0026).
    pub proof_epoch: &'a str,
    /// `checker.source_hash` — digest of the sources this build was made from.
    pub source_digest: &'a str,
    /// `checker.binary_hash` — digest of the built checker artifact.
    pub binary_digest: &'a str,
    /// `checker.toolchain` — toolchain identity that produced the build.
    pub toolchain: &'a str,
    /// `reproduce` — at least one command that re-derives this receipt.
    pub reproduce: &'a [&'a str],
}

/// A seam field, so a malformed input says *which*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SeamField {
    /// [`Seam::receipt_id`].
    ReceiptId,
    /// [`Seam::certificate_digest`].
    CertificateDigest,
    /// [`Seam::observers_digest`].
    ObserversDigest,
    /// [`Seam::proof_epoch`].
    ProofEpoch,
    /// [`Seam::source_digest`].
    SourceDigest,
    /// [`Seam::binary_digest`].
    BinaryDigest,
    /// [`Seam::toolchain`].
    Toolchain,
    /// [`Seam::reproduce`].
    Reproduce,
}

impl SeamField {
    /// The stable lower-case token naming this field in diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReceiptId => "receipt-id",
            Self::CertificateDigest => "certificate-digest",
            Self::ObserversDigest => "observers-digest",
            Self::ProofEpoch => "proof-epoch",
            Self::SourceDigest => "source-digest",
            Self::BinaryDigest => "binary-digest",
            Self::Toolchain => "toolchain",
            Self::Reproduce => "reproduce",
        }
    }
}

/// Why no receipt was produced. Closed, like every other outcome vocabulary here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptError {
    /// The verdict was not [`Verdict::Verified`], so there is no claim to record.
    NotVerified,
    /// A seam string is not printable ASCII, or is empty.
    MalformedSeamField {
        /// Which field.
        field: SeamField,
        /// What is wrong with it.
        fault: TokenFault,
    },
    /// `receipt_id` does not match the schema pattern `^receipt_[A-Za-z0-9_-]+$`.
    ReceiptIdNotWellFormed,
}

/// One checked claim, in the shape `proof-receipt.schema.json` admits.
///
/// Every field is either re-derived from the certificate's own bytes or carried from
/// the [`Seam`]; nothing is defaulted. [`Receipt::trusted_inputs`] is the boundary
/// between the two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    receipt_id: String,
    claim_text: String,
    property_hash: String,
    assurance_class: &'static str,
    trusted_components: Vec<String>,
    model_hash: String,
    assumptions_hash: String,
    observers_hash: String,
    domain_pack_hashes: Vec<String>,
    semantic_epoch: String,
    proof_epoch: String,
    certificate_kind: &'static str,
    certificate_hash: String,
    checker_version: String,
    checker_source_hash: String,
    checker_binary_hash: String,
    checker_toolchain: String,
    reproduce: Vec<String>,
}

impl Receipt {
    /// The receipt identity supplied at the seam.
    #[must_use]
    pub fn receipt_id(&self) -> &str {
        &self.receipt_id
    }

    /// The checker crate that produced this receipt.
    #[must_use]
    pub const fn checker_name(&self) -> &'static str {
        CHECKER_NAME
    }

    /// The qualified wire-epoch identity of the checker: [`WIRE_EPOCH_ID`].
    #[must_use]
    pub const fn wire_epoch_id(&self) -> &'static str {
        WIRE_EPOCH_ID
    }

    /// `checker.version`: the crate version with the wire contract as build metadata.
    #[must_use]
    pub fn checker_version(&self) -> &str {
        &self.checker_version
    }

    /// The receipt spelling of the certificate family, per the schema's enum.
    #[must_use]
    pub const fn certificate_kind(&self) -> &'static str {
        self.certificate_kind
    }

    /// The prose statement of what was established.
    #[must_use]
    pub fn claim_text(&self) -> &str {
        &self.claim_text
    }

    /// docs/03 §7's assurance class this receipt carries (RFC 0005 "Proof-producing
    /// solver policy": no bare "verified" without exposing it).
    #[must_use]
    pub const fn assurance_class(&self) -> &'static str {
        self.assurance_class
    }

    /// docs/03 §2's `trusted: Vec<TrustedComponent>` this receipt discloses — the
    /// receipt-typed counterpart of [`CheckedClaim::trusted_components`].
    #[must_use]
    pub fn trusted_components(&self) -> &[String] {
        &self.trusted_components
    }

    /// The seam fields this receipt carries without having derived them.
    ///
    /// The counterpart of [`CheckedClaim::trusted_components`], one layer out: those
    /// name what a *verdict* still trusts, these name what a *receipt* was told.
    #[must_use]
    pub const fn trusted_inputs(&self) -> &'static [&'static str] {
        &[
            "checker-build-digest",
            "checker-toolchain-identity",
            "input-certificate-digest",
            "observer-set-digest",
            "proof-epoch",
            "receipt-identity",
            "reproduction-command",
        ]
    }

    /// Serialize to the JSON form `proof-receipt.schema.json` admits.
    ///
    /// Deterministic: keys are emitted in the schema's `required` order followed by
    /// the optional ones, arrays keep the certificate's order, and no clock or path
    /// is read. Two calls on the same receipt produce identical bytes.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        field(&mut out, 1, "schema_id", SCHEMA_ID, true);
        out.push_str("  \"schema_epoch\": ");
        out.push_str(SCHEMA_EPOCH);
        out.push_str(",\n");
        field(&mut out, 1, "receipt_id", &self.receipt_id, true);

        out.push_str("  \"claim\": {\n");
        field(&mut out, 2, "kind", "safety", true);
        field(&mut out, 2, "text", &self.claim_text, true);
        field(&mut out, 2, "property_hash", &self.property_hash, true);
        field(&mut out, 2, "verdict", "established", false);
        out.push_str("  },\n");

        field(&mut out, 1, "assurance_class", self.assurance_class, true);
        array(
            &mut out,
            1,
            "trusted_components",
            &self.trusted_components,
            true,
        );

        out.push_str("  \"closure\": {\n");
        field(&mut out, 2, "model_hash", &self.model_hash, true);
        field(
            &mut out,
            2,
            "assumptions_hash",
            &self.assumptions_hash,
            true,
        );
        field(&mut out, 2, "observers_hash", &self.observers_hash, true);
        array(
            &mut out,
            2,
            "domain_pack_hashes",
            &self.domain_pack_hashes,
            false,
        );
        out.push_str("  },\n");

        out.push_str("  \"epochs\": {\n");
        field(&mut out, 2, "semantic", &self.semantic_epoch, true);
        field(&mut out, 2, "proof", &self.proof_epoch, false);
        out.push_str("  },\n");

        out.push_str("  \"transformations\": [],\n");

        out.push_str("  \"certificate\": {\n");
        field(&mut out, 2, "kind", self.certificate_kind, true);
        field(&mut out, 2, "hash", &self.certificate_hash, false);
        out.push_str("  },\n");

        out.push_str("  \"checker\": {\n");
        field(&mut out, 2, "name", CHECKER_NAME, true);
        field(&mut out, 2, "version", &self.checker_version, true);
        field(&mut out, 2, "source_hash", &self.checker_source_hash, true);
        field(&mut out, 2, "binary_hash", &self.checker_binary_hash, true);
        field(&mut out, 2, "toolchain", &self.checker_toolchain, false);
        out.push_str("  },\n");

        array(&mut out, 1, "reproduce", &self.reproduce, false);
        out.push_str("}\n");
        out
    }
}

/// Build the receipt for one verified verdict.
///
/// # Errors
///
/// [`ReceiptError::NotVerified`] when the verdict is not [`Verdict::Verified`], and
/// [`ReceiptError::MalformedSeamField`] / [`ReceiptError::ReceiptIdNotWellFormed`]
/// when a seam string is not a shape the schema admits.
pub fn receipt(verdict: &Verdict, seam: &Seam<'_>) -> Result<Receipt, ReceiptError> {
    let Some(claim) = verdict.claim() else {
        return Err(ReceiptError::NotVerified);
    };
    check_seam(seam)?;
    let envelope = claim.envelope();
    Ok(Receipt {
        receipt_id: seam.receipt_id.to_owned(),
        claim_text: claim_text(claim),
        property_hash: envelope.property_digest().as_str().to_owned(),
        assurance_class: ASSURANCE_CLASS,
        trusted_components: claim
            .trusted_components()
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
        model_hash: envelope.model_digest().as_str().to_owned(),
        assumptions_hash: envelope.assumptions_digest().as_str().to_owned(),
        observers_hash: seam.observers_digest.to_owned(),
        domain_pack_hashes: envelope
            .domain_pack_digests()
            .iter()
            .map(|digest| digest.as_str().to_owned())
            .collect(),
        semantic_epoch: envelope.semantic_epoch().as_str().to_owned(),
        proof_epoch: seam.proof_epoch.to_owned(),
        certificate_kind: schema_certificate_kind(claim.kind()),
        certificate_hash: seam.certificate_digest.to_owned(),
        checker_version: checker_version(),
        checker_source_hash: seam.source_digest.to_owned(),
        checker_binary_hash: seam.binary_digest.to_owned(),
        checker_toolchain: seam.toolchain.to_owned(),
        reproduce: seam.reproduce.iter().map(|s| (*s).to_owned()).collect(),
    })
}

/// `checker.version`: the crate version, with the wire contract as build metadata.
///
/// Semver build metadata is the one place a version string may carry an identity
/// that does not order — which is exactly what a wire epoch is.
fn checker_version() -> String {
    let magic = core::str::from_utf8(&MAGIC).unwrap_or("");
    let mut out = String::from(env!("CARGO_PKG_VERSION"));
    out.push_str("+wire.");
    out.push_str(magic);
    out.push('.');
    out.push_str(&WIRE_EPOCH.to_string());
    out
}

/// The docs/03 §7 assurance class every receipt this crate emits carries.
///
/// `continuum-kernel-core` re-derives every obligation of both families entirely
/// from the certificate's own bytes (RFC 0005 "Closed reachable set"; docs/03 §6.1):
/// there is no solver in the loop whose output this crate could trust in place of
/// checking it, so ADR-0017's `TRUSTED_SOLVER` arm never arises here. Every verified
/// claim is therefore `CHECKED_CERTIFICATE` — docs/03 §3's "small kernel checked
/// evidence" — for both [`CertificateKind::FiniteClosure`] and
/// [`CertificateKind::StateType`] alike; the value is a fact about this crate's
/// checking strategy, not a per-claim computation.
const ASSURANCE_CLASS: &str = "CHECKED_CERTIFICATE";

/// The schema's `certificate.kind` spelling for a family this crate checks.
///
/// `proof-receipt.schema.json` used to close `certificate.kind` at ten members with
/// no state-typing member, so both families this crate implements were reported as
/// `closed-set` and the distinction lived only in `claim.text` (bn-ww5ic's first
/// gap). The schema now names the state-typing family `state-type` directly, so each
/// family gets its own spelling; `claim.text` still carries the informal statement
/// of what was discharged, for a reader who wants prose alongside the typed field.
const fn schema_certificate_kind(kind: CertificateKind) -> &'static str {
    match kind {
        CertificateKind::FiniteClosure => "closed-set",
        CertificateKind::StateType => "state-type",
    }
}

/// The prose claim: the obligations discharged, the sizes traversed, what is trusted.
fn claim_text(claim: &CheckedClaim) -> String {
    let mut out = String::new();
    match claim.kind() {
        CertificateKind::FiniteClosure => {
            out.push_str("finite-closure certificate: Init ⊆ S, Post(S) ⊆ S and S ⊆ P re-derived");
        }
        CertificateKind::StateType => {
            out.push_str(
                "state-type certificate: S ⊆ P re-derived; the family carries no \
                 transition relation, so no closure obligation was discharged",
            );
        }
    }
    out.push_str(&format!(
        " over {} states ({} initial) and {} transitions; property class {}",
        claim.states(),
        claim.initial_states(),
        claim.transitions(),
        claim.property().as_str(),
    ));
    out
}

/// Shape-check every seam string. Contents are the caller's obligation; shape is not.
fn check_seam(seam: &Seam<'_>) -> Result<(), ReceiptError> {
    let strings = [
        (SeamField::ReceiptId, seam.receipt_id),
        (SeamField::CertificateDigest, seam.certificate_digest),
        (SeamField::ObserversDigest, seam.observers_digest),
        (SeamField::ProofEpoch, seam.proof_epoch),
        (SeamField::SourceDigest, seam.source_digest),
        (SeamField::BinaryDigest, seam.binary_digest),
        (SeamField::Toolchain, seam.toolchain),
    ];
    for (field, value) in strings {
        printable(field, value)?;
    }
    if seam.reproduce.is_empty() {
        return Err(ReceiptError::MalformedSeamField {
            field: SeamField::Reproduce,
            fault: TokenFault::Empty,
        });
    }
    for line in seam.reproduce {
        printable(SeamField::Reproduce, line)?;
    }
    if !well_formed_receipt_id(seam.receipt_id) {
        return Err(ReceiptError::ReceiptIdNotWellFormed);
    }
    Ok(())
}

/// Non-empty printable ASCII, space included.
///
/// Wider than `wire::Token` by exactly one byte: a reproduction command and a
/// toolchain identity contain spaces, and a digest does not. Everything outside
/// `0x20..=0x7E` stays out, so JSON escaping never has to encode a control byte.
fn printable(field: SeamField, value: &str) -> Result<(), ReceiptError> {
    if value.is_empty() {
        return Err(ReceiptError::MalformedSeamField {
            field,
            fault: TokenFault::Empty,
        });
    }
    for (offset, byte) in value.bytes().enumerate() {
        if !(0x20..=0x7E).contains(&byte) {
            return Err(ReceiptError::MalformedSeamField {
                field,
                fault: TokenFault::NonPrintable { offset, byte },
            });
        }
    }
    Ok(())
}

/// The schema's `^receipt_[A-Za-z0-9_-]+$`, spelled out.
fn well_formed_receipt_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("receipt_") else {
        return false;
    };
    !rest.is_empty()
        && rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Write `"key": "value"` at `depth`, with the JSON string escaping the schema needs.
fn field(out: &mut String, depth: usize, key: &str, value: &str, more: bool) {
    indent(out, depth);
    out.push('"');
    out.push_str(key);
    out.push_str("\": ");
    string(out, value);
    out.push_str(if more { ",\n" } else { "\n" });
}

/// Write `"key": [ … ]` at `depth`, one element per line.
fn array(out: &mut String, depth: usize, key: &str, values: &[String], more: bool) {
    indent(out, depth);
    out.push('"');
    out.push_str(key);
    out.push_str("\": [");
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        out.push('\n');
        indent(out, depth.saturating_add(1));
        string(out, value);
    }
    if !values.is_empty() {
        out.push('\n');
        indent(out, depth);
    }
    out.push(']');
    out.push_str(if more { ",\n" } else { "\n" });
}

/// A JSON string literal. Every seam and envelope value is printable ASCII, so only
/// the quote and the backslash can need escaping.
fn string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        if ch == '"' || ch == '\\' {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('"');
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::arithmetic_side_effects,
        reason = "test bodies assert on known-shaped fixtures; the no-panic covenant \
                  is a property of the shipped checker, not of its test harness"
    )]

    use super::*;
    use crate::check::check_certificate;
    use crate::fixture::Plan;

    const SEAM: Seam<'static> = Seam {
        receipt_id: "receipt_diehard-closure-0001",
        certificate_digest: "blake3:cert-diehard-closure",
        observers_digest: "blake3:diehard-observers",
        proof_epoch: "proof-2026-07",
        source_digest: "blake3:kernel-core-source",
        binary_digest: "blake3:kernel-core-binary",
        toolchain: "rustc 1.97.0 (pinned by rust-toolchain.toml)",
        reproduce: &["continuum proof check --receipt receipt_diehard-closure-0001"],
    };

    fn verified(plan: &Plan) -> Verdict {
        check_certificate(&plan.encode())
    }

    #[test]
    fn a_verified_closure_certificate_yields_the_checked_in_golden_receipt() {
        let verdict = verified(&Plan::diehard_closure());
        let built = receipt(&verdict, &SEAM).expect("the green certificate must yield a receipt");
        assert_eq!(
            built.to_json(),
            include_str!("../tests/receipt.golden.json")
        );
    }

    #[test]
    fn the_receipt_names_which_crates_wire_epoch_it_means() {
        let verdict = verified(&Plan::diehard_closure());
        let built = receipt(&verdict, &SEAM).unwrap();
        // A bare `1` would name all four kernel crates at once, which is to say none.
        assert_eq!(built.wire_epoch_id(), "continuum-kernel-core/CONTCERT/1");
        assert_eq!(built.checker_name(), "continuum-kernel-core");
        assert!(built.checker_version().ends_with("+wire.CONTCERT.1"));
        // RFC 0026 correction 17: there is no checker epoch, so the wire epoch is
        // never written into the receipt's `epochs` object.
        let json = built.to_json();
        let epochs = json.split("\"epochs\": {").nth(1).unwrap();
        let epochs = epochs.split("},").next().unwrap();
        assert!(!epochs.contains("CONTCERT"));
        assert!(!epochs.contains("checker"));
    }

    #[test]
    fn receipt_generation_is_deterministic() {
        let verdict = verified(&Plan::diehard_closure());
        let first = receipt(&verdict, &SEAM).unwrap();
        let second = receipt(&verdict, &SEAM).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.to_json(), second.to_json());
    }

    #[test]
    fn the_envelope_supplies_the_hashes_the_certificate_carries() {
        let verdict = verified(&Plan::diehard_closure());
        let json = receipt(&verdict, &SEAM).unwrap().to_json();
        assert!(json.contains("\"model_hash\": \"blake3:diehard-model\""));
        assert!(json.contains("\"property_hash\": \"blake3:diehard-typeok\""));
        assert!(json.contains("\"assumptions_hash\": \"blake3:empty-assumptions\""));
        // …and the seam supplies the ones no certificate carries.
        assert!(json.contains("\"observers_hash\": \"blake3:diehard-observers\""));
        assert!(json.contains("\"binary_hash\": \"blake3:kernel-core-binary\""));
    }

    #[test]
    fn a_state_type_receipt_does_not_claim_a_closure_obligation() {
        let verdict = verified(&Plan::diehard_type());
        let built = receipt(&verdict, &SEAM).unwrap();
        // bn-ww5ic: the schema now names this family directly instead of folding it
        // into `closed-set`.
        assert_eq!(built.certificate_kind(), "state-type");
        assert!(built.claim_text().starts_with("state-type certificate"));
        assert!(built.claim_text().contains("no closure obligation"));
    }

    #[test]
    fn assurance_class_and_trusted_components_are_typed_not_prose() {
        let verdict = verified(&Plan::diehard_closure());
        let built = receipt(&verdict, &SEAM).unwrap();
        assert_eq!(built.assurance_class(), "CHECKED_CERTIFICATE");
        assert_eq!(
            built.trusted_components(),
            [
                "certificate-model-correspondence",
                "envelope-digest-binding"
            ]
        );
        // The facts moved out of the prose sentence entirely.
        assert!(!built.claim_text().contains("trusted components"));
        let json = built.to_json();
        assert!(json.contains("\"assurance_class\": \"CHECKED_CERTIFICATE\""));
        assert!(json.contains("\"trusted_components\": ["));
        assert!(json.contains("\"certificate-model-correspondence\""));
        assert!(json.contains("\"envelope-digest-binding\""));
    }

    #[test]
    fn a_rejected_certificate_has_no_receipt() {
        let mut plan = Plan::diehard_closure();
        plan.trailing = vec![0x00];
        let verdict = verified(&plan);
        assert!(matches!(verdict, Verdict::Rejected(_)));
        assert_eq!(receipt(&verdict, &SEAM), Err(ReceiptError::NotVerified));
    }

    #[test]
    fn an_unsupported_certificate_has_no_receipt() {
        let mut plan = Plan::diehard_closure();
        plan.wire_epoch = 2;
        plan.schema_epoch = 2;
        let verdict = verified(&plan);
        assert!(matches!(verdict, Verdict::Unsupported(_)));
        assert_eq!(receipt(&verdict, &SEAM), Err(ReceiptError::NotVerified));
    }

    #[test]
    fn every_seam_field_is_shape_checked() {
        let verdict = verified(&Plan::diehard_closure());
        type Break = fn(&mut Seam<'static>);
        let cases: [(SeamField, Break); 7] = [
            (SeamField::ReceiptId, |s| s.receipt_id = ""),
            (SeamField::CertificateDigest, |s| s.certificate_digest = ""),
            (SeamField::ObserversDigest, |s| s.observers_digest = ""),
            (SeamField::ProofEpoch, |s| s.proof_epoch = ""),
            (SeamField::SourceDigest, |s| s.source_digest = ""),
            (SeamField::BinaryDigest, |s| s.binary_digest = ""),
            (SeamField::Toolchain, |s| s.toolchain = ""),
        ];
        for (field, break_it) in cases {
            let mut seam = SEAM;
            break_it(&mut seam);
            assert_eq!(
                receipt(&verdict, &seam),
                Err(ReceiptError::MalformedSeamField {
                    field,
                    fault: TokenFault::Empty
                }),
                "an empty {} must be refused",
                field.as_str()
            );
        }
    }

    #[test]
    fn a_seam_string_with_a_control_byte_is_refused() {
        let verdict = verified(&Plan::diehard_closure());
        let mut seam = SEAM;
        seam.toolchain = "rustc\n1.97.0";
        assert_eq!(
            receipt(&verdict, &seam),
            Err(ReceiptError::MalformedSeamField {
                field: SeamField::Toolchain,
                fault: TokenFault::NonPrintable {
                    offset: 5,
                    byte: b'\n'
                }
            })
        );
    }

    #[test]
    fn a_receipt_without_a_reproduction_command_is_refused() {
        let verdict = verified(&Plan::diehard_closure());
        let mut seam = SEAM;
        seam.reproduce = &[];
        assert_eq!(
            receipt(&verdict, &seam),
            Err(ReceiptError::MalformedSeamField {
                field: SeamField::Reproduce,
                fault: TokenFault::Empty
            })
        );
    }

    #[test]
    fn a_receipt_id_outside_the_schema_pattern_is_refused() {
        let verdict = verified(&Plan::diehard_closure());
        for bad in [
            "diehard-0001",
            "receipt_",
            "receipt_bad id",
            "receipt_bad.id",
        ] {
            let mut seam = SEAM;
            seam.receipt_id = bad;
            assert!(
                receipt(&verdict, &seam).is_err(),
                "{bad:?} is not a receipt id the schema admits"
            );
        }
        let mut seam = SEAM;
        seam.receipt_id = "receipt_A-z_0-9";
        assert!(receipt(&verdict, &seam).is_ok());
    }

    #[test]
    fn the_trusted_inputs_are_exactly_the_seam() {
        let verdict = verified(&Plan::diehard_closure());
        let built = receipt(&verdict, &SEAM).unwrap();
        assert_eq!(built.trusted_inputs().len(), 7);
        let mut sorted = built.trusted_inputs().to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, built.trusted_inputs());
    }

    #[test]
    fn json_string_escaping_covers_the_quote_and_the_backslash() {
        let mut out = String::new();
        string(&mut out, r#"a"b\c"#);
        assert_eq!(out, r#""a\"b\\c""#);
    }
}
