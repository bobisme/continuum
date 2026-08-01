//! INV-014 receipts: what this checker established, and what it was told (PR 9).
//!
//! > Lean version, library closure, theorem hashes, axioms, certificate schema, and
//! > checker identity are recorded.
//! >
//! > — `notes/plan/plan.md`, INV-014 "Version-explicit proof"
//!
//! The receipt shape is fixed by `notes/plan/schemas/proof-receipt.schema.json`
//! (RFC 0024). [`Receipt::to_json`] emits an instance of that schema, and
//! `crates/continuum-kernel-sat/tests/receipt.golden.json` is the byte-for-byte
//! output for the three-variable refutation fixture; `tools/check_kernel_covenant.py`
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
//! [`WIRE_EPOCH_ID`], `continuum-kernel-sat/CONTSATC/1` — and it is carried in
//! `checker.version`, never in the receipt's `epochs` object:
//!
//! > **There is no checker epoch.** `EpochSet` has no `checker` member. […] checker
//! > identity is the `proof` epoch for Lean-backed checkers and engine identity for
//! > native checkers.
//! >
//! > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, correction 17
//!
//! # The seam, and why it is not a digest this crate computes
//!
//! [`Seam`] is the receipt's trusted-input inventory: every field of a proof receipt
//! that a self-contained certificate checker structurally cannot derive. It is
//! modelled the way `CheckedClaim::trusted_components` already models the envelope
//! digests — carried, labelled, and named as trusted, never invented. The build
//! digest and toolchain identity belong to the build system, the input digest to the
//! producer's content-addressing layer (a hash function inside the trusted base
//! would be trusted code, and ADR-0013 makes hash equality *not* identity in
//! certified lanes), and `observers_hash`, `epochs.proof`, `receipt_id` and
//! `reproduce` appear nowhere in the wire form.
//!
//! # Why `claim.kind` is on the seam here and not in `continuum-kernel-core`
//!
//! A closed-set certificate carries its own safety property, so the core checker
//! knows the claim is a safety claim. An LRAT refutation does not: it establishes
//! that *this CNF has no model*, and whether that discharges a safety obligation, a
//! refinement obligation or a reduction side-condition is a fact about the encoding —
//! precisely the `formula-model-correspondence` component
//! [`CheckedClaim::trusted_components`] already reports as trusted. Deriving a claim
//! kind here would be the kernel asserting something it cannot see, so the caller
//! supplies it and the receipt records that it was supplied.
//!
//! # Only a verified verdict yields a receipt
//!
//! [`receipt`] returns [`ReceiptError::NotVerified`] for [`Verdict::Rejected`] and
//! [`Verdict::Unsupported`] — including the RAT steps this build reserves. A receipt
//! records a discharged claim; a rejection is a statement about an artifact and an
//! unsupported feature is not a verdict about the model at all (INV-008).

use crate::verdict::{CertificateKind, CheckedClaim, TokenFault, Verdict};
use crate::wire::{MAGIC, WIRE_EPOCH};

/// Artifact-class identity of the schema this receipt is written against.
pub const SCHEMA_ID: &str = "https://continuum.dev/schema/proof-receipt.json";

/// Schema epoch of `notes/plan/schemas/proof-receipt.schema.json`.
pub const SCHEMA_EPOCH: &str = "1";

/// This crate's name, as it appears in a receipt's `checker.name`.
pub const CHECKER_NAME: &str = "continuum-kernel-sat";

/// The qualified spelling of this crate's wire epoch.
///
/// `<crate>/<magic>/<epoch>`. The crate name and the eight-byte magic are what
/// distinguish this `1` from the `1` of the three sibling kernel crates; see the
/// module documentation.
pub const WIRE_EPOCH_ID: &str = "continuum-kernel-sat/CONTSATC/1";

/// The `claim.kind` vocabulary `proof-receipt.schema.json` closes.
///
/// Supplied at the seam: see the module documentation for why a refutation cannot
/// name the obligation it discharges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClaimKind {
    /// A safety obligation.
    Safety,
    /// A liveness obligation.
    Liveness,
    /// A refinement obligation.
    Refinement,
    /// A reduction side-condition.
    Reduction,
    /// A conformance obligation.
    Conformance,
    /// A hyperproperty obligation.
    Hyperproperty,
}

impl ClaimKind {
    /// The schema spelling of this claim kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Safety => "safety",
            Self::Liveness => "liveness",
            Self::Refinement => "refinement",
            Self::Reduction => "reduction",
            Self::Conformance => "conformance",
            Self::Hyperproperty => "hyperproperty",
        }
    }
}

/// The receipt fields a certificate checker cannot derive, and is therefore given.
///
/// Every field is a *trusted input*: the receipt records it,
/// [`Receipt::trusted_inputs`] names it, and this crate never invents a value for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seam<'a> {
    /// `receipt_id` — must match the schema's `^receipt_[A-Za-z0-9_-]+$`.
    pub receipt_id: &'a str,
    /// `claim.kind` — the obligation the refutation discharges.
    pub claim_kind: ClaimKind,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    receipt_id: String,
    claim_kind: ClaimKind,
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

    /// The obligation class the caller said this refutation discharges.
    #[must_use]
    pub const fn claim_kind(&self) -> ClaimKind {
        self.claim_kind
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
    #[must_use]
    pub const fn trusted_inputs(&self) -> &'static [&'static str] {
        &[
            "checker-build-digest",
            "checker-toolchain-identity",
            "claim-obligation-class",
            "input-certificate-digest",
            "observer-set-digest",
            "proof-epoch",
            "receipt-identity",
            "reproduction-command",
        ]
    }

    /// Serialize to the JSON form `proof-receipt.schema.json` admits.
    ///
    /// Deterministic: no clock, no path, no map iteration order.
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
        field(&mut out, 2, "kind", self.claim_kind.as_str(), true);
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
        claim_kind: seam.claim_kind,
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
/// A [`Verdict::Verified`] LRAT refutation has already had every derived clause
/// re-derived by unit propagation from its declared antecedents (see the module
/// documentation, and [`CheckedClaim::trusted_components`]'s note that "the solver is
/// deliberately absent from this list"). ADR-0017's `TRUSTED_SOLVER` is precisely the
/// case where that replay did *not* happen; a receipt only exists once it has, so
/// every receipt this crate emits is `CHECKED_CERTIFICATE` — docs/03 §3's "small
/// kernel checked evidence" — by construction.
const ASSURANCE_CLASS: &str = "CHECKED_CERTIFICATE";

/// The schema's `certificate.kind` spelling. `lrat` is a member of the enum outright.
const fn schema_certificate_kind(kind: CertificateKind) -> &'static str {
    match kind {
        CertificateKind::Lrat => "lrat",
    }
}

/// The prose claim: the obligations discharged, the sizes traversed, what is trusted.
fn claim_text(claim: &CheckedClaim) -> String {
    let mut out = String::from(
        "lrat certificate: the carried CNF has no model — every derived clause \
         re-derived by unit propagation from its declared antecedents, ending in the \
         empty clause",
    );
    out.push_str(&format!(
        "; {} variables, {} input clauses, {} derived, {} deleted, {} propagations",
        claim.variables(),
        claim.input_clauses(),
        claim.derived_clauses(),
        claim.deleted_clauses(),
        claim.propagations(),
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

/// Write `"key": "value"` at `depth`.
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
    use crate::fixture::{Plan, PlannedStep};

    const SEAM: Seam<'static> = Seam {
        receipt_id: "receipt_three-variable-refutation-0001",
        claim_kind: ClaimKind::Safety,
        certificate_digest: "blake3:cert-three-variable-refutation",
        observers_digest: "blake3:three-variable-observers",
        proof_epoch: "proof-2026-07",
        source_digest: "blake3:kernel-sat-source",
        binary_digest: "blake3:kernel-sat-binary",
        toolchain: "rustc 1.97.0 (pinned by rust-toolchain.toml)",
        reproduce: &["continuum proof check --receipt receipt_three-variable-refutation-0001"],
    };

    fn verdict(plan: &Plan) -> Verdict {
        check_certificate(&plan.encode())
    }

    #[test]
    fn a_verified_refutation_yields_the_checked_in_golden_receipt() {
        let verdict = verdict(&Plan::three_variable_refutation());
        let built = receipt(&verdict, &SEAM).expect("the green refutation must yield a receipt");
        assert_eq!(
            built.to_json(),
            include_str!("../tests/receipt.golden.json")
        );
    }

    #[test]
    fn the_receipt_names_which_crates_wire_epoch_it_means() {
        let verdict = verdict(&Plan::three_variable_refutation());
        let built = receipt(&verdict, &SEAM).unwrap();
        assert_eq!(built.wire_epoch_id(), "continuum-kernel-sat/CONTSATC/1");
        assert_eq!(built.checker_name(), "continuum-kernel-sat");
        assert!(built.checker_version().ends_with("+wire.CONTSATC.1"));
        // RFC 0026 correction 17: there is no checker epoch, so the wire epoch is
        // never written into the receipt's `epochs` object.
        let json = built.to_json();
        let epochs = json.split("\"epochs\": {").nth(1).unwrap();
        let epochs = epochs.split("},").next().unwrap();
        assert!(!epochs.contains("CONTSATC"));
        assert!(!epochs.contains("checker"));
    }

    #[test]
    fn receipt_generation_is_deterministic() {
        let verdict = verdict(&Plan::three_variable_refutation());
        let first = receipt(&verdict, &SEAM).unwrap();
        let second = receipt(&verdict, &SEAM).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.to_json(), second.to_json());
    }

    #[test]
    fn the_obligation_class_is_carried_from_the_seam_not_invented() {
        let verdict = verdict(&Plan::three_variable_refutation());
        for kind in [
            ClaimKind::Safety,
            ClaimKind::Liveness,
            ClaimKind::Refinement,
            ClaimKind::Reduction,
            ClaimKind::Conformance,
            ClaimKind::Hyperproperty,
        ] {
            let mut seam = SEAM;
            seam.claim_kind = kind;
            let built = receipt(&verdict, &seam).unwrap();
            assert_eq!(built.claim_kind(), kind);
            assert!(
                built
                    .to_json()
                    .contains(&format!("\"kind\": \"{}\"", kind.as_str()))
            );
        }
    }

    #[test]
    fn a_rejected_certificate_has_no_receipt() {
        let mut plan = Plan::three_variable_refutation();
        plan.trailing = vec![0x00];
        let verdict = verdict(&plan);
        assert!(matches!(verdict, Verdict::Rejected(_)));
        assert_eq!(receipt(&verdict, &SEAM), Err(ReceiptError::NotVerified));
    }

    #[test]
    fn an_unsupported_rat_step_has_no_receipt() {
        let mut plan = Plan::three_variable_refutation();
        plan.steps.insert(0, PlannedStep::RawKind(3));
        let verdict = verdict(&plan);
        assert!(matches!(verdict, Verdict::Unsupported(_)));
        assert_eq!(receipt(&verdict, &SEAM), Err(ReceiptError::NotVerified));
    }

    #[test]
    fn every_seam_field_is_shape_checked() {
        let verdict = verdict(&Plan::three_variable_refutation());
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
        let verdict = verdict(&Plan::three_variable_refutation());
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
    fn a_receipt_id_outside_the_schema_pattern_is_refused() {
        let verdict = verdict(&Plan::three_variable_refutation());
        for bad in [
            "three-variable",
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
    }

    #[test]
    fn a_receipt_without_a_reproduction_command_is_refused() {
        let verdict = verdict(&Plan::three_variable_refutation());
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
    fn assurance_class_and_trusted_components_are_typed_not_prose() {
        let verdict = verdict(&Plan::three_variable_refutation());
        let built = receipt(&verdict, &SEAM).unwrap();
        assert_eq!(built.assurance_class(), "CHECKED_CERTIFICATE");
        assert_eq!(
            built.trusted_components(),
            ["envelope-digest-binding", "formula-model-correspondence"]
        );
        // The facts moved out of the prose sentence entirely.
        assert!(!built.claim_text().contains("trusted components"));
        let json = built.to_json();
        assert!(json.contains("\"assurance_class\": \"CHECKED_CERTIFICATE\""));
        assert!(json.contains("\"trusted_components\": ["));
        assert!(json.contains("\"envelope-digest-binding\""));
        assert!(json.contains("\"formula-model-correspondence\""));
    }
}
