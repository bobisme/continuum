"""docs/19 §9 "Security validation" — TEST-9-07 "signature/provenance verification"
(bn-4yta). Left unclaimed by `s9_security_tests.py` (TEST-9-01 … TEST-9-06, bn-35hb) —
see that module's `unclaimed` evidence key. This module claims TEST-9-07 without
editing a line of the sibling module, `tsys.py`, or the `Justfile`.

# What "signature/provenance verification" splits into, and what each half gets

docs/19 §9's seventh bullet names two things in one phrase. Searched independently in
`crates/`:

1. **Cryptographic signatures — signed and verified on production daemon paths.**
   bn-2ee4c (ADR-0054) added `continuum-evidence::signing`: Ed25519 (`ed25519-dalek`
   `=2.2.0`, strict verification) over a canonical envelope that binds the artifact's
   ADR-0013 canonical bytes, the kind, and the signer. Its `SignatureVerifier` is total
   and fails closed in CI mode. bn-1hape put the *signing* half on a production path:
   - `continuum-security::entropy::OsEntropy` is the production `KeyEntropy` capability,
     in a boundary crate that no semantic-core crate reaches;
   - `continuum-security::keystore::LocalKeystore` persists the solo-developer key,
     minted on first use through `SigningRegistry::mint` (an audit record), `0600`/`0700`,
     zeroized, with typed refusal of a corrupt, truncated, oversize, incomplete, or exposed
     store;
   - `continuumd`'s `evidence.link`, the daemon's one receipt producer, signs the
     receipt's canonical bytes through `SigningRegistry::sign` (`ReceiptSigner`) before
     publishing, and refuses to publish when its identity is not active.
   bn-3glnv (protocol 3.8) put the *verifying* half on production paths too:
   - `continuumd`'s `SigningAuthority` (`daemon/signing.rs`) is the one production holder
     of `SignatureVerifier`: `signing.verify` returns typed provenance for a receipt, a
     bundle body, or a domain pack; `intent.import_bundle` verifies a bundle and applies
     the standing facts it carries — never one that could widen trust, never one about
     the daemon's own key — only when it verifies under them; and `intent.accept` naming
     a bundle calls `check_acceptance_chain`, which calls `verify_for_ci_acceptance`,
     fails closed, and refuses a retired signer;
   - the bundle and pack producers sign their own kinds there
     (`intent.export_bundle`, `signing.sign_pack`), and `evidence.get` returns a
     receipt's signature.
   `_check_production_signing_paths` binds every one of those call sites in the real
   source, and fails if the verifier appears in an unbound `src/` file. The named Rust
   tests in `crates/continuumd/tests/daemon_signing.rs` drive each path through
   `Daemon::dispatch`. bn-18w74 made the authority survive a restart: the keystore
   persists every change the daemon makes (keys minted over the wire, own and adopted
   signer links, pre-signed revocations, and the own-key set, recorded apart from the
   registry's signers) through the `SigningCustody` capability, atomically, and
   `Builder::launch_signing` builds a daemon from it, re-validating everything before a
   key is used; `crates/continuumd/tests/signing_custody.rs` drives each restart path.
   What stays out of scope is stated in `ABSENCE`, and none of it is verification.
2. **Content-addressed provenance — real, landed, non-stub.** Two production
   mechanisms bind an artifact's claimed identity to its actual bytes and reject a
   forged, tampered, or substituted claim:
   - `continuum-value::identity` (ADR-0013): `ContentIdentity` and `Digest256` make a
     digest a pure function of canonical bytes; a non-canonical encoding, a malformed
     digest token, or an uppercase/mis-length token is a typed rejection, never a
     silent acceptance.
   - `continuum-workspace::publication` (`Published<H>`, `ReferenceStore`): the store
     computes an artifact's identity from its own content — "a lying publisher cannot
     file content under someone else's name" (the module's own doc comment) — and
     `Published::attest` mints a "this artifact is published" witness only from a
     receipt whose handle the offered name actually equals. Two different contents
     colliding on one claimed identity abort rather than conflate; two publications of
     identical content converge to one identity rather than mint a second.
   Both are drift-tied below to their real, committed source, and both are
   corroborated by real, non-`#[ignore]`d Rust `#[test]` functions.

TEST-9-07 is `enforced` as of bn-3glnv: the evidence names real Rust tests that
exercise the production verification path, `_rust_test_present` checks each exists and
is not ignored, and the Python port of the decision order is drift-tied to `fn check`.
`_check_production_signing_paths` keeps the production signing and verifying paths
bound, so the status cannot outlive them.

Certificate wire-form checking (INV-004) was also searched:
`crates/continuum-engine-reference/src/certificate.rs` writes the certificate and
`crates/continuum-kernel-core/src/check.rs` checks it from wire bytes alone, exactly
as plan §20 requires. But the kernel's own test states the boundary this module must
not cross past: `envelope_digests_are_carried_labels_not_facts_the_kernel_can_check`
(`check.rs`) relabels `model_digest` on a certificate and asserts the checker still
*verifies* — "changing a digest byte produces a different verified claim, never a
rejection, because nothing in the certificate lets the kernel recompute it." So the
certificate checker is not itself a provenance-forgery detector; binding a digest to a
real artifact is `continuum-workspace::publication`'s job (the module doc names this
outright: "Binding those digests to real artifacts is the receipt's obligation"). This
module therefore claims the publication layer, not the certificate checker, as its
provenance evidence, and does not claim `envelope_digests_are_carried_labels…` as
positive evidence — it is cited only in `ABSENCE` below.

# Method (same discipline as the sibling module)

`check_fixture` is a Python reference model of three real, narrow properties, each
drift-tied to its real source by extracting a constant or checking a literal phrase is
still present at run time (never hand-copied). None of this executes
`Value::encode`, `Blake3Hasher`, or `ReferenceStore::publish` (no cargo) — the models
are structural (equality, length, character class), not a reimplementation of the real
hash functions, and the real, non-`#[ignore]`d Rust tests named below are the evidence
that the real code has the property the model states.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

import tsys

SECTION = 9
TITLE = "Security validation"

# tools/test-policy/sections/s9_security_tests_07.py -> repo root
ROOT = Path(__file__).resolve().parents[3]
PUBLICATION_RS_PATH = ROOT / "crates/continuum-workspace/src/publication.rs"
IDENTITY_RS_PATH = ROOT / "crates/continuum-value/src/identity.rs"
CHECK_RS_PATH = ROOT / "crates/continuum-kernel-core/src/check.rs"
SIGNING_RS_PATH = ROOT / "crates/continuum-evidence/src/signing.rs"
SIGNING_TESTS_PATH = ROOT / "crates/continuum-evidence/tests/signing_identities.rs"
ENTROPY_RS_PATH = ROOT / "crates/continuum-security/src/entropy.rs"
ENTROPY_ISOLATION_PATH = ROOT / "crates/continuum-security/tests/entropy_isolation.rs"
KEYSTORE_TESTS_PATH = ROOT / "crates/continuum-security/tests/keystore.rs"
KEYSTORE_PROCESSES_PATH = ROOT / "crates/continuum-security/tests/keystore_processes.rs"
KEYSTORE_RS_PATH = ROOT / "crates/continuum-security/src/keystore.rs"
RECEIPT_SIGNING_PATH = ROOT / "crates/continuumd/tests/receipt_signing.rs"
DAEMON_SIGNING_RS_PATH = ROOT / "crates/continuumd/src/daemon/signing.rs"
DAEMON_SIGNING_PATH = ROOT / "crates/continuumd/tests/daemon_signing.rs"
CUSTODY_PATH = ROOT / "crates/continuumd/tests/signing_custody.rs"
CARGO_TOML_PATHS = sorted((ROOT / "crates").glob("*/Cargo.toml")) + [ROOT / "Cargo.toml"]

OBLIGATIONS = {
    "TEST-9-07": "signature/provenance verification.",
}

RULES: dict[str, str] = {
    "publication-name-substitution-not-rejected-detected": "TEST-9-07",
    "identity-collision-not-aborted-detected": "TEST-9-07",
    "distinct-artifacts-conflated-detected": "TEST-9-07",
    "identical-content-not-converged-detected": "TEST-9-07",
    "digest-token-tamper-not-rejected-detected": "TEST-9-07",
    "unverified-signature-accepted-detected": "TEST-9-07",
    "verified-signature-rejected-detected": "TEST-9-07",
    "ci-acceptance-failed-open-detected": "TEST-9-07",
}

STATUS: dict[str, str] = {
    "TEST-9-07": "enforced",
}

_BOUNDARY_07 = (
    "`_check_publication_attest` ports `PublishedName::names`/`Published::attest` "
    "(`publication.rs`: `fn names(&self, handle: &ArtifactHandle) -> bool { self == "
    "handle }`) as plain equality on (class, identity) — the real predicate needs no "
    "hash to state, so the port is exact, not approximate. `_check_identity_outcome` "
    "ports the store's convergence/collision/distinctness trichotomy structurally "
    "(same content implies same identity; different content sharing a claimed "
    "identity is a collision that must abort; different content with different "
    "identity stands as two artifacts) — this is the property the real "
    "`ContentIdentifier::identify` doc contract states ('a pure function … two calls "
    "with equal arguments … must return equal handles', INV-005/INV-006) and that "
    "`an_identity_collision_aborts_rather_than_conflating_two_artifacts` and "
    "`distinct_artifacts_never_share_identity` corroborate; it is independent of "
    "which hash algorithm the real `ContentIdentifier` fixture uses; this module "
    "never computes one. `_check_digest_token` ports `Digest256::from_token` "
    "(`identity.rs`) as a length-and-character-class check, drift-tied by extracting "
    "`DIGEST_BITS` at run time and confirming the `DIGEST_LEN * 2`/`DIGEST_BITS / 8` "
    "arithmetic phrases are still present, so a change to the digest width is "
    "noticed, not just a change to the accept/reject outcome. What this cannot do: "
    "compute a real `blake3-256` digest, run `Value::encode`'s canonical-encoding "
    "rules, or exercise `ReferenceStore` concurrency itself (no cargo) — only the "
    "structural properties named above, which is what the named real Rust tests "
    "exercise for real."
)
_BOUNDARY_07_SIGNATURE = (
    "`_check_signature_verdict` ports `SignatureVerifier::check` (`signing.rs`) as its "
    "decision order over eight observable facts — present, well-formed, kind matches, "
    "authentic under the claimed key, registry current against the authoritative head, "
    "signer standing known, signer not revoked, signer allowed for the kind — and "
    "the two policies over it: `verify` (any failure is typed unverified provenance with "
    "the first failing reason) and `verify_for_ci_acceptance` (any failure is "
    "`AcceptanceChainInvalid`). The port is exact about order and outcome, and "
    "`_check_signing_drift` confirms at run time that the real `fn check` body still "
    "tests those facts in that order and that the real verifier still calls "
    "`verify_strict`. What it cannot do: compute an Ed25519 signature or a canonical "
    "encoding (no cargo). That is what the named Rust tests do for real, including the "
    "RFC 8032 §7.1 known-answer vectors and a pinned envelope signature."
)
_ABSENCE_07 = (
    "Outside this obligation, stated so it is not read as covered. Held intent bundles "
    "and the record of which contracts an import entered are not persisted: after a "
    "restart, `intent.accept` naming a bundle held before it fails closed "
    "(`AcceptanceChainInvalid`) until the bundle is imported again. A verifier that "
    "never received a bundle carrying a revocation cannot know of it; adoption is "
    "monotone, so once any bundle carries it the revocation is never unlearned. research/35 "
    "now names an eleventh red-team class, `forged or unattested signing lineage` "
    "(bn-1uspo): its three-per-outcome floor runs inside the generic corpus at protocol 3.2, "
    "on operations the corpus already served, because a `Case` names no protocol version and "
    "the six signing-wire operations are refused below 3.8 by the codec, not the capability "
    "check the corpus measures. `daemon_signing.rs`'s "
    "`every_signing_wire_operation_refuses_or_answers_typed_for_an_unprivileged_reader` and "
    "`the_signing_wires_refusal_does_not_move_with_a_hostile_corpus_payload` drive the six "
    "signing-wire operations and `intent.import_bundle` directly, at 3.8, against the real "
    "daemon, with the corpus's own payloads; `g2_injection_corpus_evidence.rs`'s "
    "`SIGNING_WIRE_PROBES` still stands in for them there, because that file's shared runner "
    "fixes protocol 3.2 for the whole corpus in one pass and has no per-case protocol floor — "
    "closing that gap is a follow-up. `check.rs`'s "
    "`envelope_digests_are_carried_labels_not_facts_the_kernel_can_check` still states that "
    "the certificate checker cannot recompute a relabeled digest; that is by design "
    "(INV-004), and the certificate checker does not link the signature crate."
)
BOUNDARIES: dict[str, list[str]] = {"TEST-9-07": [_BOUNDARY_07, _BOUNDARY_07_SIGNATURE]}
ABSENCE: dict[str, str] = {"TEST-9-07": _ABSENCE_07}

# ---------------------------------------------------------------------------
# Real Rust #[test] evidence (textual, no cargo — s6_fuzzing_targets.py's
# _rust_test_present discipline, reused verbatim rather than re-implemented).
# ---------------------------------------------------------------------------

RUST_TESTS: list[tuple[Path, str]] = [
    (PUBLICATION_RS_PATH, "published_attests_only_the_receipts_own_handle"),
    (PUBLICATION_RS_PATH, "published_is_invariant_under_the_handles_serialization_round_trip"),
    (PUBLICATION_RS_PATH, "a_receipt_names_the_identity_the_store_derived"),
    (PUBLICATION_RS_PATH, "distinct_artifacts_never_share_identity"),
    (PUBLICATION_RS_PATH, "an_identity_collision_aborts_rather_than_conflating_two_artifacts"),
    (PUBLICATION_RS_PATH, "concurrent_identical_publication_yields_one_identity_and_every_receipt"),
    (IDENTITY_RS_PATH, "non_canonical_bytes_are_not_an_identity"),
    (IDENTITY_RS_PATH, "malformed_digest_tokens_are_typed_errors"),
    (IDENTITY_RS_PATH, "digest_tokens_round_trip"),
    (IDENTITY_RS_PATH, "blake3_matches_the_published_test_vectors"),
    (SIGNING_RS_PATH, "ed25519_matches_the_rfc_8032_test_vectors"),
    (SIGNING_RS_PATH, "signature_tokens_round_trip_and_reject_malformed_text"),
    (SIGNING_RS_PATH, "weak_and_non_point_public_keys_are_not_signer_identities"),
    (SIGNING_RS_PATH, "an_overlong_token_or_record_is_refused_before_decoding"),
    (SIGNING_RS_PATH, "a_signature_record_fits_the_decode_bound"),
    (SIGNING_TESTS_PATH, "a_signer_with_unknown_standing_is_unverified_not_active"),
    (SIGNING_TESTS_PATH, "a_stale_registry_missing_a_revocation_cannot_verify"),
    (SIGNING_TESTS_PATH, "an_empty_registry_verifies_no_signature"),
    (SIGNING_TESTS_PATH, "a_registry_replays_from_its_persisted_audit_log"),
    (SIGNING_TESTS_PATH, "replay_refuses_an_illegal_or_resequenced_log"),
    (SIGNING_TESTS_PATH, "restore_rederives_only_a_minted_key"),
    (ENTROPY_RS_PATH, "os_entropy_supplies_distinct_nonzero_seeds"),
    (ENTROPY_ISOLATION_PATH, "no_semantic_core_crate_can_reach_the_os_entropy_source"),
    (ENTROPY_ISOLATION_PATH, "the_entropy_device_is_named_in_exactly_one_source_file"),
    (ENTROPY_ISOLATION_PATH, "continuum_evidence_names_no_entropy_source"),
    (KEYSTORE_TESTS_PATH, "first_use_mints_once_with_os_entropy_and_reopening_restores_the_same_key"),
    (KEYSTORE_TESTS_PATH, "the_store_is_private_to_its_owner"),
    (KEYSTORE_TESTS_PATH, "an_exposed_key_or_state_file_is_refused"),
    (KEYSTORE_TESTS_PATH, "a_corrupt_key_file_is_refused_and_never_re_minted"),
    (KEYSTORE_TESTS_PATH, "a_missing_state_or_key_file_is_refused_and_never_re_minted"),
    (KEYSTORE_TESTS_PATH, "a_truncated_extended_or_garbage_state_file_fails_closed"),
    (KEYSTORE_TESTS_PATH, "an_oversize_state_file_is_refused_before_it_is_read"),
    (KEYSTORE_TESTS_PATH, "a_store_of_the_earlier_layout_is_refused_and_never_minted_over"),
    (KEYSTORE_TESTS_PATH, "a_persisted_rotation_round_trips_and_removes_the_retired_key"),
    (KEYSTORE_TESTS_PATH, "a_refused_persist_leaves_the_previous_state"),
    (KEYSTORE_TESTS_PATH, "the_sweep_removes_what_a_crash_leaves_and_keeps_the_held_key"),
    (KEYSTORE_TESTS_PATH, "no_error_or_debug_output_carries_key_material"),
    (KEYSTORE_TESTS_PATH, "an_absent_store_opens_as_absent_and_a_failed_source_mints_nothing"),
    (KEYSTORE_TESTS_PATH, "a_group_or_world_writable_store_directory_is_refused_and_not_re_minted"),
    (KEYSTORE_TESTS_PATH, "a_store_owned_by_another_user_is_refused"),
    (KEYSTORE_TESTS_PATH, "a_symlinked_key_or_state_file_is_refused"),
    (KEYSTORE_TESTS_PATH, "a_symlinked_store_directory_is_refused"),
    (KEYSTORE_TESTS_PATH, "a_world_writable_non_sticky_ancestor_is_refused"),
    (KEYSTORE_PROCESSES_PATH, "a_hostile_audit_log_is_refused_within_a_memory_limit"),
    (KEYSTORE_RS_PATH, "swap_after_open_reads_the_original_descriptor"),
    (KEYSTORE_RS_PATH, "o_nofollow_refuses_a_symlink_at_open"),
    (KEYSTORE_RS_PATH, "audit_records_fit_the_record_bound"),
    (RECEIPT_SIGNING_PATH, "a_receipt_evidence_link_produces_is_signed_and_verifies"),
    (RECEIPT_SIGNING_PATH, "a_receipt_tampered_after_production_does_not_verify"),
    (RECEIPT_SIGNING_PATH, "a_daemon_without_a_signer_publishes_unsigned_and_a_verifier_says_so"),
    (RECEIPT_SIGNING_PATH, "a_revoked_signing_identity_refuses_the_link_rather_than_publishing_unsigned"),
    (SIGNING_TESTS_PATH, "sign_verify_round_trips_for_every_signed_kind_from_wire_bytes"),
    (SIGNING_TESTS_PATH, "a_tampered_payload_downgrades_to_signature_mismatch"),
    (SIGNING_TESTS_PATH, "a_signature_under_the_wrong_key_does_not_verify"),
    (SIGNING_TESTS_PATH, "a_revoked_signers_signatures_downgrade_and_it_can_no_longer_sign"),
    (SIGNING_TESTS_PATH, "a_rotated_key_keeps_its_signatures_verifiable_but_cannot_sign_again"),
    (SIGNING_TESTS_PATH, "a_signer_outside_the_allowed_set_or_its_kinds_is_not_trusted"),
    (SIGNING_TESTS_PATH, "a_kind_relabel_is_refused_before_and_after_the_kind_field_is_rewritten"),
    (SIGNING_TESTS_PATH, "an_unverifiable_signature_downgrades_to_typed_unverified_provenance_not_fail_open"),
    (SIGNING_TESTS_PATH, "the_ci_acceptance_check_fails_closed_on_every_unverified_outcome"),
    (SIGNING_TESTS_PATH, "signatures_are_deterministic_and_pinned"),
    (SIGNING_TESTS_PATH, "the_local_key_is_minted_on_first_use_once_and_audited"),
    (SIGNING_TESTS_PATH, "a_lost_key_is_revoked_and_superseded_through_linked_audit_records"),
    (SIGNING_TESTS_PATH, "an_allowed_signers_set_round_trips_through_its_canonical_value"),
    # The production verification path (bn-3glnv, protocol 3.8), driven through
    # `Daemon::dispatch`.
    (DAEMON_SIGNING_PATH, "an_exported_bundle_imports_and_its_chain_accepts_the_proposal"),
    (DAEMON_SIGNING_PATH, "accept_fails_closed_on_every_broken_chain"),
    (DAEMON_SIGNING_PATH, "an_importer_with_its_own_key_still_adopts_and_accepts"),
    (DAEMON_SIGNING_PATH, "an_imported_proposal_is_accepted_only_through_a_verified_bundle"),
    (DAEMON_SIGNING_PATH, "an_imported_revision_may_not_move_a_field_its_predecessor_protects"),
    (DAEMON_SIGNING_PATH, "an_acceptance_needs_its_predecessor_accepted_here"),
    (DAEMON_SIGNING_PATH, "the_ci_check_refuses_a_retired_signer"),
    (DAEMON_SIGNING_PATH, "a_full_registry_can_still_revoke"),
    (DAEMON_SIGNING_PATH, "a_mint_signs_only_the_kinds_it_names"),
    (DAEMON_SIGNING_PATH, "the_signing_wire_is_refused_below_protocol_3_8"),
    (DAEMON_SIGNING_PATH, "a_revocation_carried_by_a_bundle_is_adopted_and_a_stale_bundle_cannot_resurrect_the_key"),
    (DAEMON_SIGNING_PATH, "a_retired_key_adopts_no_facts"),
    (DAEMON_SIGNING_PATH, "a_known_signer_learns_its_attested_rotation_and_revocation"),
    (DAEMON_SIGNING_PATH, "a_signer_cannot_claim_a_known_or_unseen_pinned_key_without_its_signature"),
    (DAEMON_SIGNING_PATH, "a_link_about_this_daemons_own_key_is_never_adopted"),
    (DAEMON_SIGNING_PATH, "link_order_and_replay_do_not_change_what_is_adopted"),
    (DAEMON_SIGNING_PATH, "a_loss_recovery_never_travels"),
    (DAEMON_SIGNING_PATH, "a_bundle_whose_own_signature_fails_adopts_no_link"),
    (DAEMON_SIGNING_PATH, "a_held_key_revocation_always_leaves_room_for_its_replacement"),
    (DAEMON_SIGNING_PATH, "a_bundle_that_rotates_one_key_twice_is_refused_whole"),
    (DAEMON_SIGNING_PATH, "an_unpinned_successor_is_never_introduced"),
    (DAEMON_SIGNING_PATH, "a_record_is_accepted_only_as_its_chain_signed_it"),
    (DAEMON_SIGNING_PATH, "an_acceptance_by_a_rotated_or_revoked_signer_does_not_vouch"),
    (DAEMON_SIGNING_PATH, "a_local_acceptance_at_3_8_records_a_verifiable_chain"),
    (DAEMON_SIGNING_PATH, "a_signed_local_acceptance_names_only_the_admitted_principal_at_the_daemons_time"),
    (DAEMON_SIGNING_PATH, "every_order_of_a_rotation_chain_gives_the_same_standing_and_facts"),
    (DAEMON_SIGNING_PATH, "a_prelearned_first_key_ends_rotated_and_fails_ci_acceptance"),
    (DAEMON_SIGNING_PATH, "a_cycle_or_a_shared_successor_is_refused_whole"),
    (DAEMON_SIGNING_RS_PATH, "the_topology_check_is_bounded_at_the_link_bound"),
    (DAEMON_SIGNING_PATH, "a_receipt_node_keeps_its_first_bytes_and_its_first_signature"),
    (DAEMON_SIGNING_PATH, "staging_never_overwrites_content_under_a_colliding_commitment"),
    (DAEMON_SIGNING_PATH, "an_install_that_would_leave_a_compromise_unrecoverable_is_refused"),
    (SIGNING_TESTS_PATH, "a_signer_link_is_attested_by_every_key_it_concerns_and_nothing_else"),
    (SIGNING_TESTS_PATH, "a_link_signature_is_never_an_artifact_signature"),
    (DAEMON_SIGNING_PATH, "an_import_whose_contract_collides_with_a_held_one_is_refused_before_anything_changes"),
    (DAEMON_SIGNING_PATH, "accept_never_takes_a_signature_over_one_body_for_another_under_the_same_identity"),
    (DAEMON_SIGNING_PATH, "a_bundle_identity_that_names_other_bytes_is_refused_on_import_and_export"),
    (DAEMON_SIGNING_PATH, "a_receipt_node_identity_that_names_another_receipt_is_refused_before_signing"),
    (DAEMON_SIGNING_PATH, "an_import_refused_for_quota_adopts_nothing"),
    (DAEMON_SIGNING_PATH, "an_export_pins_only_the_active_bundle_signers"),
    (DAEMON_SIGNING_PATH, "the_version_gate_is_exactly_the_idls_3_8_operations"),
    (DAEMON_SIGNING_PATH, "truncated_extended_or_oversize_bundles_are_refused_whole"),
    (DAEMON_SIGNING_PATH, "a_bundle_whose_contract_is_not_its_declared_identity_is_refused"),
    (DAEMON_SIGNING_PATH, "import_is_idempotent_and_never_raises_status"),
    (DAEMON_SIGNING_PATH, "signing_verify_is_total_and_typed"),
    (DAEMON_SIGNING_PATH, "a_signer_allowed_for_one_kind_is_not_verified_for_another"),
    (DAEMON_SIGNING_PATH, "a_receipt_signature_travels_on_evidence_get_and_verifies_over_the_wire"),
    (DAEMON_SIGNING_PATH, "a_domain_pack_signed_over_the_wire_verifies_and_a_tampered_one_does_not"),
    (DAEMON_SIGNING_PATH, "the_daemons_bundle_signature_matches_the_library_signing_the_same_body"),
    (DAEMON_SIGNING_PATH, "a_revoked_held_key_refuses_to_sign_anything"),
    (DAEMON_SIGNING_PATH, "mint_rotate_revoke_move_the_registry_forward_only"),
    (DAEMON_SIGNING_PATH, "a_replayed_rotation_does_not_rotate_twice"),
    (DAEMON_SIGNING_PATH, "every_signing_wire_operation_is_refused_to_a_scoped_grant"),
    (DAEMON_SIGNING_PATH, "the_writes_are_refused_without_the_privilege"),
    (DAEMON_SIGNING_PATH, "no_answer_fault_or_debug_output_carries_key_material"),
    # The signing authority across a restart (bn-18w74), launched from the real keystore.
    (CUSTODY_PATH, "a_restart_keeps_rotations_revocations_and_adopted_links"),
    (CUSTODY_PATH, "own_and_adopted_keys_stay_apart_across_a_restart"),
    (CUSTODY_PATH, "an_install_without_custody_state_owns_only_the_held_key"),
    (CUSTODY_PATH, "a_corrupt_truncated_or_oversize_store_fails_closed_at_launch"),
    (CUSTODY_PATH, "a_state_that_does_not_hold_is_refused_before_any_key_is_used"),
    (CUSTODY_PATH, "a_restored_state_without_room_to_recover_is_refused"),
    (CUSTODY_PATH, "an_exposed_or_symlinked_store_is_refused_at_launch"),
    (CUSTODY_PATH, "a_custody_that_refuses_a_write_changes_nothing_and_stops_signing"),
    (CUSTODY_PATH, "the_launch_sweeps_a_retired_key_left_by_a_crash"),
    (CUSTODY_PATH, "no_launch_error_or_debug_output_carries_key_material"),
    (CUSTODY_PATH, "the_keystore_bounds_are_the_daemons"),
    (CUSTODY_PATH, "a_replacement_minted_near_the_bound_survives_a_restart"),
    (CUSTODY_PATH, "a_signer_installed_over_a_launched_custody_writes_nothing"),
    (CUSTODY_PATH, "a_second_launch_on_a_held_store_is_refused"),
    (CUSTODY_PATH, "a_state_missing_what_a_rotation_leaves_is_refused"),
    (CUSTODY_PATH, "the_restored_authority_matches_the_library_running_the_same_operations"),
    (KEYSTORE_TESTS_PATH, "a_store_held_as_a_custody_is_locked_against_every_other_writer"),
    (KEYSTORE_TESTS_PATH, "a_hard_linked_key_file_is_refused"),
    (KEYSTORE_TESTS_PATH, "a_custody_state_survives_the_serialization_round_trip"),
    (KEYSTORE_TESTS_PATH, "a_write_reports_which_side_of_the_rename_it_failed_on"),
    (KEYSTORE_TESTS_PATH, "concurrent_writes_through_one_handle_are_serialized"),
    (KEYSTORE_TESTS_PATH, "a_second_handle_in_process_is_refused_while_one_holds_the_store"),
    (KEYSTORE_PROCESSES_PATH, "a_spawned_process_inherits_nothing_and_is_refused_the_store"),
    (KEYSTORE_RS_PATH, "every_store_descriptor_is_close_on_exec"),
    (KEYSTORE_RS_PATH, "a_handle_owned_by_another_process_is_refused_before_it_touches_a_path"),
    (KEYSTORE_TESTS_PATH, "an_inherited_handle_is_refused_at_every_window_its_mutex_can_be_held"),
    (KEYSTORE_TESTS_PATH, "an_inherited_handle_is_refused_its_first_load"),
    (KEYSTORE_TESTS_PATH, "a_symlink_in_or_through_a_writable_directory_is_refused"),
    (KEYSTORE_TESTS_PATH, "a_trusted_symlink_chain_reaches_the_store"),
    (KEYSTORE_TESTS_PATH, "a_symlink_swapped_after_validation_cannot_redirect_a_write"),
    (KEYSTORE_TESTS_PATH, "a_symlink_in_the_ancestry_is_itself_held_to_the_owner_rule"),
    (KEYSTORE_TESTS_PATH, "a_store_directory_another_user_owns_is_refused"),
    (KEYSTORE_TESTS_PATH, "a_hard_linked_symlink_in_the_path_is_refused"),
    (KEYSTORE_RS_PATH, "an_ancestor_another_user_owns_or_can_write_is_refused"),
    (CUSTODY_PATH, "unreconciled_standing_decides_no_trust"),
    (CUSTODY_PATH, "a_recorded_outcome_unknown_is_never_replayed_to_a_3_8_client"),
    (DAEMON_SIGNING_RS_PATH, "an_unreconciled_custody_vouches_for_no_acceptance"),
    (DAEMON_SIGNING_PATH, "a_bundle_acceptance_fails_closed_while_the_importers_custody_is_unreconciled"),
    (CUSTODY_PATH, "every_answer_agrees_with_every_state_a_crash_can_leave"),
    (CUSTODY_PATH, "a_custody_daemon_refuses_signing_writes_below_3_9"),
    (CUSTODY_PATH, "every_record_keeps_its_link_or_entry_across_a_restart"),
    (CUSTODY_PATH, "a_compromised_own_key_without_its_revocation_link_is_refused"),
]


def _rust_test_present(path: Path, fn_name: str) -> tuple[bool, str]:
    """Textual, no-cargo check: `fn <fn_name>(` exists in `path`, immediately preceded
    (skipping only blank lines and `//`/`///` comments) by an attribute run that
    includes `#[test]` and no `#[ignore]`. Same discipline as the sibling module and
    `s6_fuzzing_targets.py`."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        return False, f"could not read {path}: {exc}"
    marker = f"fn {fn_name}("
    at = text.find(marker)
    if at == -1:
        return False, f"{path}: no `fn {fn_name}(` found"
    attrs: list[str] = []
    for line in reversed(text[:at].splitlines()):
        s = line.strip()
        if s.startswith("#["):
            attrs.append(s)
            continue
        if s == "" or s.startswith("///") or s.startswith("//"):
            continue
        break
    if not any(a.startswith("#[test") for a in attrs):
        return False, f"{path}: `fn {fn_name}` is not attributed #[test]"
    if any("ignore" in a for a in attrs):
        return False, f"{path}: `fn {fn_name}` carries #[ignore]"
    return True, ""


def _finding(rule: str, subject: str, message: str) -> dict[str, str]:
    return tsys.Finding(rule, RULES[rule], subject, message).as_json()


# ---------------------------------------------------------------------------
# Drift checks against real source.
# ---------------------------------------------------------------------------

_PUBLISHED_NAME_PHRASE = "fn names(&self, handle: &ArtifactHandle) -> bool {\n        self == handle"
_IDENTITY_COLLISION_PHRASE = "AbortReason::IdentityCollision"
_PURE_FUNCTION_DOC_PHRASE = "publisher cannot file content under someone else's name"


def _check_publication_drift() -> list[str]:
    problems: list[str] = []
    text = PUBLICATION_RS_PATH.read_text(encoding="utf-8")
    if _PUBLISHED_NAME_PHRASE not in text:
        problems.append(
            f"drift: {PUBLICATION_RS_PATH} no longer declares PublishedName::names as plain handle equality"
        )
    if _IDENTITY_COLLISION_PHRASE not in text:
        problems.append(f"drift: {PUBLICATION_RS_PATH} no longer declares {_IDENTITY_COLLISION_PHRASE}")
    if _PURE_FUNCTION_DOC_PHRASE not in text:
        problems.append(f"drift: {PUBLICATION_RS_PATH} no longer states its content-addressing guarantee verbatim")
    return problems


_DIGEST_BITS_RE = re.compile(r"pub const DIGEST_BITS: usize = (\d+);")
_DIGEST_LEN_ARITHMETIC_PHRASE = "pub const DIGEST_LEN: usize = DIGEST_BITS / 8;"
_DIGEST_TOKEN_LEN_ARITHMETIC_PHRASE = "pub const DIGEST_TOKEN_LEN: usize = DIGEST_LEN * 2;"
_DIGEST_TOKEN_CHARCLASS_PHRASE = "'0'..='9' | 'a'..='f'"


def _extract_digest_token_len() -> int:
    text = IDENTITY_RS_PATH.read_text(encoding="utf-8")
    m = _DIGEST_BITS_RE.search(text)
    if not m:
        raise ValueError(f"drift: no DIGEST_BITS constant found in {IDENTITY_RS_PATH}")
    if _DIGEST_LEN_ARITHMETIC_PHRASE not in text or _DIGEST_TOKEN_LEN_ARITHMETIC_PHRASE not in text:
        raise ValueError(f"drift: {IDENTITY_RS_PATH} no longer derives DIGEST_TOKEN_LEN as DIGEST_BITS / 8 * 2")
    if _DIGEST_TOKEN_CHARCLASS_PHRASE not in text:
        raise ValueError(f"drift: {IDENTITY_RS_PATH} no longer restricts a digest token to {_DIGEST_TOKEN_CHARCLASS_PHRASE!r}")
    bits = int(m.group(1))
    return (bits // 8) * 2


_SIGNATURE_DEP_RE = re.compile(r"^(ed25519[\w-]*|dalek[\w-]*|hmac|p256|secp256\w*|rsa|ring|signature)\s*[=.]", re.M)
_SIGNATURE_CRATE = "ed25519-dalek"
_SIGNATURE_PIN_PHRASE = 'ed25519-dalek = { version = "=2.2.0", default-features = false, features = ["zeroize"] }'
_SIGNATURE_CONSUMERS = {"crates/continuum-evidence/Cargo.toml"}
_CHECKER_MANIFESTS = {
    "crates/continuum-certificate/Cargo.toml",
    "crates/continuum-kernel-core/Cargo.toml",
    "crates/continuum-kernel-sat/Cargo.toml",
    "crates/continuum-kernel-smt/Cargo.toml",
    "crates/continuum-kernel-temporal/Cargo.toml",
}


def _signature_dependency_declarations() -> dict[str, list[str]]:
    """Reads every real, committed `Cargo.toml` (not a model of one) and returns, per
    manifest, the signature or MAC crates it declares."""
    found: dict[str, list[str]] = {}
    for path in CARGO_TOML_PATHS:
        text = path.read_text(encoding="utf-8")
        hits = [m.group(1) for m in _SIGNATURE_DEP_RE.finditer(text)]
        if hits:
            found[str(path.relative_to(ROOT))] = hits
    return found


def _check_signature_dependency_placement() -> list[str]:
    """The signature crate is pinned exactly at the workspace root, declared by
    `continuum-evidence` alone, and by no certificate-checker crate (INV-004, ADR-0054
    D3). A direct read of the real manifests, like `s9_security_tests.py`'s TEST-9-01
    calling the real governance gates."""
    problems: list[str] = []
    root_text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    if _SIGNATURE_PIN_PHRASE not in root_text:
        problems.append(f"drift: the workspace Cargo.toml no longer pins {_SIGNATURE_PIN_PHRASE!r}")
    declared = _signature_dependency_declarations()
    consumers = {path for path in declared if path != "Cargo.toml"}
    for path in sorted(consumers - _SIGNATURE_CONSUMERS):
        problems.append(f"{path} declares {declared[path]!r}; ADR-0054 D3 names continuum-evidence alone")
    for path in sorted(consumers & _CHECKER_MANIFESTS):
        problems.append(f"{path} is a certificate-checker crate and declares a signature crate (INV-004)")
    for path in sorted(_SIGNATURE_CONSUMERS - consumers):
        problems.append(f"{path} no longer declares {_SIGNATURE_CRATE}: the signing half has no implementation")
    return problems


# ---------------------------------------------------------------------------
# TEST-9-07: signature verification (SignatureVerifier::check and its two policies).
# ---------------------------------------------------------------------------

# The facts `fn check` tests, in the order it tests them, each with the unverified
# reason its failure yields. `authentic` is `verify_strict`; `revoked` fails when true.
_SIGNATURE_STEPS: list[tuple[str, str, str]] = [
    ("present", "UnverifiedReason::Unsigned", "unsigned"),
    ("well_formed", "UnverifiedReason::Malformed", "malformed"),
    ("kind_matches", "UnverifiedReason::KindMismatch", "kind-mismatch"),
    ("authentic", "UnverifiedReason::SignatureMismatch", "signature-mismatch"),
    ("registry_current", "UnverifiedReason::StandingStale", "standing-stale"),
    ("standing_known", "UnverifiedReason::StandingUnknown", "standing-unknown"),
    ("not_revoked", "UnverifiedReason::SignerRevoked", "signer-revoked"),
    ("allowed", "UnverifiedReason::SignerNotAllowed", "signer-not-allowed"),
]
_FACTS = [fact for fact, _, _ in _SIGNATURE_STEPS]
# `Arguments::SigningRegistry` and `Payload::SigningRegistry` — declared, matched as
# `Self::…`, and constructed — are the protocol 3.8 wire variants of `signing.registry`,
# which carry no key and no registry; they are not the library type and are not callers
# of it. The guards bind to the `SigningRegistry` branch alone, so `Self::LocalSigner` and
# the other names still count.
_SIGNING_API_RE = re.compile(
    r"\b(?:(?<!Arguments::)(?<!Payload::)(?<!Self::)"
    r"SigningRegistry(?!\((?:SigningRegistryRequest|SigningRegistryResponse)\))"
    r"|SignatureVerifier|LocalKeyring|ReceiptSigner|LocalSigner)\b"
)
# The production call sites bn-1hape and bn-3glnv added, each bound by phrases that must
# still be present.
_SIGNING_PATHS: dict[str, list[str]] = {
    "crates/continuum-security/src/entropy.rs": [
        "impl KeyEntropy for OsEntropy",
        "OS_ENTROPY_PATH",
    ],
    "crates/continuum-security/src/keystore.rs": [
        ".mint(actor, &mut capture)",
        # bn-18w74: every later change, written whole and renamed into place, and a
        # held key's secret written before the state that names it.
        "fn replace_state(",
        "fs::rename(&temp, &state).map_err(io(\"replacing the state file\"))",
        # cr-33e464: the rename is the commit point, and the result says which side.
        "marked_here = mark_pending(store, owner)?;",
        # cr-1dc5ii: one writer, owned by one process.
        "owned_here(store)?;",
        # cr-2qu5zr: every component of the store's ancestry is trusted, and re-checked
        # right before the commit point.
        "fn trusted_ancestry(path: &Path, owner: u32) -> Result<bool, KeystoreError> {",
        "trusted_symlink(&current, &next, &metadata, owner)?;",
        "if metadata.nlink() != 1 {",
        "recheck(store, owner)?;\n            fs::rename(&temp, &state)",
        "return Err(CustodyWrite::Unconfirmed(error));",
        "write_key(store, owner, held, seed)?;",
        "impl SigningCustody for LocalKeystore {",
        ".mode(0o600)",
        ".mode(0o700)",
        "registry.restore(seed)",
        # cr-3l3n47: descriptor-bound reads and a bounded, record-at-a-time replay.
        ".custom_flags(O_NOFOLLOW)",
        "(opened.dev(), opened.ino()) != (examined.dev(), examined.ino())",
        "return Err(KeystoreError::SymlinkedStore);",
        "if count > MAX_AUDIT_RECORDS {",
        ".replay_record(parsed)",
    ],
    "crates/continuumd/src/daemon/mod.rs": [
        "pub fn receipt_signer(mut self, signer: ReceiptSigner)",
        "self.state.signing_mut().install(registry, key, restored)",
        # bn-18w74: the launcher re-validates a restored custody before the key is used.
        "pub fn launch_signing<C>(",
        # cr-1dc5ii round 2: a replayed or fresh answer is held to the negotiated version.
        "if !errors::defined_at(",
        "let signer = ReceiptSigner::restored(key, state).map_err(LaunchRefusal::Install)?;",
        "signing::validate_custody(&state, signer.identity()).map_err(InstallRefusal::Custody)?;",
    ],
    # bn-3glnv: the signing authority — the one production verifier outside
    # `continuum-evidence`, and the receipt, bundle, and pack signers.
    "crates/continuumd/src/daemon/signing.rs": [
        "self.sign_as(Kind::Receipt, artifact)",
        ".sign_as(Kind::IntentBundle, &signed_bytes_identity(body_bytes))",
        ".sign_as(Kind::DomainPack, &signed_bytes_identity(&request.pack))",
        "if !self.allowed.permits(key.identity(), kind) {",
        "SignatureVerifier::new(&self.allowed, &self.registry, &self.head)",
        ".verify_for_ci_acceptance(",
        "fn check_acceptance_chain(",
        "if verified.standing() != &VerifiedStanding::Active {",
        "Provenance::Unverified(_) => return Err(outcome_of(&provenance)),",
        "fn with_carried_facts(",
        "verify_acceptance_chain(",
        ".map_err(AcceptanceFault::Chain)?;",
        "if !signature.authenticates(Kind::IntentBundle, &body)",
        "|| !links_are_attested(&bundle.body().links)",
        ".rotate_attested(current, &actor, &mut capture)",
        # bn-18w74: every change is recorded through the custody before it takes effect.
        "authority.commit(checkpoint, Some(&identity), minted)?;",
        "Err(CustodyWrite::Unconfirmed(())) => {",
        "Self::Unconfirmed => Err(outcome_unknown()),",
        # cr-1dc5ii round 2: unreconciled standing decides no trust.
        "return Err(AcceptanceFault::CustodyUnreconciled);",
        "return Err(SignatureOutcome::StandingStale);",
        "pub fn validate_custody(",
        ".and_then(|key| authority.registry.attest_revocation(key).ok())",
        "authority.room(4, Reserve::HeldKey)?;",
        "if entry.contract != claim.contract {",
        "fn holds_other_bundle(",
    ],
    # cr-2unxyh: the RFC 0037 A1 acceptance chain — the statement each element signs and
    # the in-order check; its verifier is `signing.rs`'s.
    "crates/continuumd/src/daemon/acceptance.rs": [
        "pub const ACCEPTANCE_DOMAIN: &str = \"continuum.intent-acceptance.v1\";",
        "(name(\"previous\"), Value::bytes(previous.to_vec())),",
        "match verify(&statement.identity(&previous), &signature) {",
        "Provenance::Verified(_) => return Err(ChainFault::Retired),",
        "return Err(ChainFault::NotLast);",
    ],
    "crates/continuumd/src/daemon/state.rs": ["fn record_receipt_signature("],
    "crates/continuumd/src/daemon/evidence.rs": [
        ".sign_receipt(&signed_receipt_identity(&staged.content))",
        "state.record_receipt_signature(receipt_handle.clone(), signature)",
    ],
}
_SIGN_BEFORE_PUBLISH = (".sign_receipt(&signed_receipt_identity(&staged.content))", ".publish(ArtifactClass::Evidence, staged.content.clone(), &token)")
_VERIFIER_RE = re.compile(r"\bSignatureVerifier\b")
_VERIFIER_HOME = "crates/continuumd/src/daemon/signing.rs"
# `intent.accept` reaches the fail-closed check (bn-3glnv).
_ACCEPT_SITE = ("crates/continuumd/src/daemon/intent.rs", ".check_acceptance_chain(bundle, &request.proposal, &claim, &actor)")


def _check_production_signing_paths() -> tuple[dict[str, int], list[str]]:
    """Binds the real production signing and verifying paths.

    Bound: every phrase in `_SIGNING_PATHS` is present, `evidence.link` signs before it
    publishes, and `intent.accept` calls the fail-closed chain check. Allowed callers: a
    `src/` file outside `continuum-evidence` that names the signing API must be one of
    `_SIGNING_PATHS`, and only `_VERIFIER_HOME` may name `SignatureVerifier`."""
    counts = {"bound_phrases": 0, "production_callers": 0}
    problems: list[str] = []
    for rel, phrases in _SIGNING_PATHS.items():
        path = ROOT / rel
        if not path.is_file():
            problems.append(f"{rel} is missing: the bound signing path is gone")
            continue
        text = path.read_text(encoding="utf-8")
        for phrase in phrases:
            if phrase in text:
                counts["bound_phrases"] += 1
            else:
                problems.append(f"drift: {rel} no longer contains {phrase!r}")
    evidence_text = (ROOT / "crates/continuumd/src/daemon/evidence.rs").read_text(encoding="utf-8")
    sign_at, publish_at = (evidence_text.find(p) for p in _SIGN_BEFORE_PUBLISH)
    if sign_at == -1 or publish_at == -1 or sign_at > publish_at:
        problems.append("evidence.link no longer signs the receipt before it publishes it")
    accept_rel, accept_phrase = _ACCEPT_SITE
    if accept_phrase not in (ROOT / accept_rel).read_text(encoding="utf-8"):
        problems.append("intent.accept no longer runs the fail-closed chain check")
    else:
        counts["bound_phrases"] += 1
    for path in sorted((ROOT / "crates").glob("*/src/**/*.rs")):
        rel = path.relative_to(ROOT).as_posix()
        if rel.startswith("crates/continuum-evidence/"):
            continue
        text = path.read_text(encoding="utf-8")
        if _SIGNING_API_RE.search(text):
            counts["production_callers"] += 1
            if rel not in _SIGNING_PATHS:
                problems.append(f"{rel} calls the signing library outside the bound paths; bind it here")
        if _VERIFIER_RE.search(text) and rel != _VERIFIER_HOME:
            problems.append(f"{rel} names SignatureVerifier outside {_VERIFIER_HOME}; bind it here")
    return counts, problems


_CHECK_FN_RE = re.compile(r"    fn check\(\n(.*?)\n    }\n", re.S)


def _check_signing_drift() -> list[str]:
    """Ties `_SIGNATURE_STEPS` to the real `fn check` body: each reason's first mention
    (and the `verify_strict` call for authenticity) must appear in the port's order.
    `Malformed` is decided one layer up, in `verify_encoded`, before `check` runs."""
    problems: list[str] = []
    text = SIGNING_RS_PATH.read_text(encoding="utf-8")
    m = _CHECK_FN_RE.search(text)
    if not m:
        return [f"drift: {SIGNING_RS_PATH} no longer declares `fn check(` on SignatureVerifier"]
    body = m.group(1)
    anchors = [
        ("UnverifiedReason::Unsigned", body),
        ("UnverifiedReason::KindMismatch", body),
        (".verify_strict(", body),
        ("UnverifiedReason::StandingStale", body),
        ("UnverifiedReason::StandingUnknown", body),
        ("UnverifiedReason::SignerRevoked", body),
        ("UnverifiedReason::SignerNotAllowed", body),
    ]
    last = -1
    for needle, hay in anchors:
        at = hay.find(needle)
        if at == -1:
            problems.append(f"drift: `fn check` no longer mentions {needle}")
            continue
        if at < last:
            problems.append(f"drift: `fn check` tests {needle} out of the ported order")
        last = at
    if "UnverifiedReason::Malformed(error)" not in text or "fn verify_encoded(" not in text:
        problems.append("drift: `verify_encoded` no longer maps a decode failure to UnverifiedReason::Malformed")
    if "Provenance::Unverified(unverified) => Err(AcceptanceChainInvalid {" not in text:
        problems.append("drift: `verify_for_ci_acceptance` no longer maps every unverified outcome to AcceptanceChainInvalid")
    if "fn check(" in text and "-> Provenance {" not in text:
        problems.append("drift: `verify` no longer returns a total Provenance")
    tests_text = SIGNING_TESTS_PATH.read_text(encoding="utf-8")
    if "const PINNED_SIGNATURE: &str = \"ed25519:" not in tests_text:
        problems.append(f"drift: {SIGNING_TESTS_PATH} no longer pins a known-answer envelope signature")
    return problems


def _signature_reference(facts: dict[str, bool], mode: str) -> str:
    """The real outcome: `verified`, `unverified:<reason>`, or `ci-rejected:<reason>`."""
    for fact, _, reason in _SIGNATURE_STEPS:
        if not facts[fact]:
            return f"{'ci-rejected' if mode == 'ci' else 'unverified'}:{reason}"
    return "verified"


def _check_signature_verdict(payload: dict) -> list[dict[str, str]]:
    facts = {fact: bool(payload[fact]) for fact in _FACTS}
    mode = payload["mode"]
    expected = _signature_reference(facts, mode)
    claimed = payload["claimed_outcome"]
    if claimed == expected:
        return []
    if claimed == "verified":
        rule = "ci-acceptance-failed-open-detected" if mode == "ci" else "unverified-signature-accepted-detected"
    elif expected == "verified":
        rule = "verified-signature-rejected-detected"
    else:
        rule = "ci-acceptance-failed-open-detected" if mode == "ci" else "unverified-signature-accepted-detected"
    return [
        _finding(
            rule,
            ",".join(f"{k}={int(v)}" for k, v in facts.items()) + f";mode={mode}",
            f"real SignatureVerifier outcome is {expected!r} (checks run in the order "
            f"{', '.join(_FACTS)}), fixture claims {claimed!r}",
        )
    ]


# ---------------------------------------------------------------------------
# TEST-9-07: publication-receipt attestation (Published<H>::attest).
# ---------------------------------------------------------------------------


def _check_publication_attest(payload: dict) -> list[dict[str, str]]:
    receipt = (payload["receipt_class"], payload["receipt_identity"])
    offered = (payload["offered_class"], payload["offered_identity"])
    accepted_ref = receipt == offered
    claimed_accepted = payload["claimed_accepted"]
    if accepted_ref == claimed_accepted:
        return []
    return [
        _finding(
            "publication-name-substitution-not-rejected-detected",
            f"{offered[0]}:{offered[1]} against receipt {receipt[0]}:{receipt[1]}",
            f"offered name {offered!r} against a receipt issued for {receipt!r}: real "
            f"Published::attest acceptance is {accepted_ref} (PublishedName::names is plain "
            f"handle equality), fixture claims accepted={claimed_accepted}",
        )
    ]


# ---------------------------------------------------------------------------
# TEST-9-07: content-addressed identity — convergence, collision, distinctness.
# ---------------------------------------------------------------------------


def _expected_identity_outcome(same_content: bool, same_identity: bool) -> str:
    if same_content and same_identity:
        return "converged"
    if not same_content and same_identity:
        return "aborted"
    if not same_content and not same_identity:
        return "distinct"
    raise ValueError(
        "contradictory payload: identical content claiming different identities would mean "
        "ContentIdentifier::identify is not a pure function (INV-005/INV-006) — this module "
        "never constructs that combination"
    )


_OUTCOME_RULE = {
    "aborted": "identity-collision-not-aborted-detected",
    "distinct": "distinct-artifacts-conflated-detected",
    "converged": "identical-content-not-converged-detected",
}


def _check_identity_outcome(payload: dict) -> list[dict[str, str]]:
    same_content = payload["content_a"] == payload["content_b"]
    same_identity = payload["identity_a"] == payload["identity_b"]
    expected = _expected_identity_outcome(same_content, same_identity)
    claimed = payload["claimed_outcome"]
    if claimed == expected:
        return []
    rule = _OUTCOME_RULE[expected]
    return [
        _finding(
            rule,
            f"{payload['identity_a']}/{payload['identity_b']}",
            f"content {'equal' if same_content else 'distinct'}, claimed identity "
            f"{'equal' if same_identity else 'distinct'}: real store outcome is {expected!r} "
            f"(a lying publisher cannot file content under someone else's name), fixture "
            f"claims {claimed!r}",
        )
    ]


# ---------------------------------------------------------------------------
# TEST-9-07: digest token parsing (Digest256::from_token).
# ---------------------------------------------------------------------------

_HEX_DIGITS = set("0123456789abcdef")


def _digest_token_reference(token: str, token_len: int) -> bool:
    if any(c not in _HEX_DIGITS for c in token):
        return False
    return len(token) == token_len


def _check_digest_token(payload: dict) -> list[dict[str, str]]:
    token_len = _extract_digest_token_len()
    token = payload["token"]
    accepted_ref = _digest_token_reference(token, token_len)
    claimed_accepted = payload["claimed_accepted"]
    if accepted_ref == claimed_accepted:
        return []
    return [
        _finding(
            "digest-token-tamper-not-rejected-detected",
            token,
            f"token {token!r} (expected length {token_len}): real Digest256::from_token "
            f"acceptance is {accepted_ref}, fixture claims accepted={claimed_accepted}",
        )
    ]


# ---------------------------------------------------------------------------
# Fixtures dispatch.
# ---------------------------------------------------------------------------


def check_fixture(system: Any) -> list[dict[str, str]]:
    if not isinstance(system, dict) or "kind" not in system:
        return [
            _finding(
                "publication-name-substitution-not-rejected-detected",
                "<payload>",
                "payload must be an object with a 'kind'",
            )
        ]
    kind = system["kind"]
    payload = {k: v for k, v in system.items() if k != "kind"}
    if kind == "publication-attest":
        return _check_publication_attest(payload)
    if kind == "identity-outcome":
        return _check_identity_outcome(payload)
    if kind == "digest-token":
        return _check_digest_token(payload)
    if kind == "signature-verdict":
        return _check_signature_verdict(payload)
    return [
        _finding(
            "publication-name-substitution-not-rejected-detected",
            "<payload>",
            f"unknown fixture kind {kind!r}",
        )
    ]


# ---------------------------------------------------------------------------
# The real run.
# ---------------------------------------------------------------------------


def real_run() -> dict[str, Any]:
    failures: list[str] = []
    corpus: dict[str, int] = {}
    mutants = {"applied": 0, "detected": 0}

    def bump(key: str, n: int = 1) -> None:
        corpus[key] = corpus.get(key, 0) + n

    def mutate(detected: bool, subject: str) -> None:
        mutants["applied"] += 1
        if detected:
            mutants["detected"] += 1
        else:
            failures.append(f"mutant on {subject} was not detected: the checker reported it clean")

    def need(key: str, what: str) -> None:
        if corpus.get(key, 0) == 0:
            failures.append(f"corpus exercises no {what} ({key} = 0): the pass would be vacuous")

    # -- signature crate placement: real manifests ------------------------------------
    bump("cargo_toml_files_scanned", len(CARGO_TOML_PATHS))
    failures.extend(_check_signature_dependency_placement())
    path_counts, path_problems = _check_production_signing_paths()
    failures.extend(path_problems)
    bump("production_signing_phrases_bound", path_counts["bound_phrases"])
    bump("production_signing_callers", path_counts["production_callers"])
    need("production_signing_phrases_bound", "a bound production signing call site")
    need("production_signing_callers", "a production caller of the signing library")
    need("cargo_toml_files_scanned", "a real Cargo.toml to scan")

    # -- signature-verdict: every fact combination, both policies -----------------------
    failures.extend(_check_signing_drift())
    for bits in range(1 << len(_FACTS)):
        facts = {fact: bool(bits >> i & 1) for i, fact in enumerate(_FACTS)}
        for mode in ("provenance", "ci"):
            expected = _signature_reference(facts, mode)
            bump("signature_cases")
            bump(f"signature_{expected.split(':')[0].replace('-', '_')}")
            payload = {**facts, "mode": mode}
            if _check_signature_verdict({**payload, "claimed_outcome": expected}):
                failures.append(f"signature {payload!r}: the reference's own honest claim was flagged")
            wrong = "unverified:signature-mismatch" if expected == "verified" else "verified"
            if mode == "ci" and expected == "verified":
                wrong = "ci-rejected:signature-mismatch"
            mut = _check_signature_verdict({**payload, "claimed_outcome": wrong})
            mutate(bool(mut), f"signature {payload!r} (claimed {wrong!r} instead of {expected!r})")
    for key, what in (
        ("signature_verified", "a signature every check accepts"),
        ("signature_unverified", "a downgraded (unverified) signature"),
        ("signature_ci_rejected", "a CI fail-closed rejection"),
    ):
        need(key, what)

    # -- publication-attest ------------------------------------------------------------
    drift = _check_publication_drift()
    failures.extend(drift)
    attest_cases = [
        ("ev", "aaaa1111", "ev", "aaaa1111", True),
        ("ev", "aaaa1111", "ev", "bbbb2222", False),
        ("ev", "aaaa1111", "task", "aaaa1111", False),
        ("crash", "cp7m3x9", "crash", "cp7m3x9", True),
    ]
    for rc, ri, oc, oi, accepted in attest_cases:
        bump("attest_cases")
        if not accepted:
            bump("attest_substitution_cases")
        payload = {"receipt_class": rc, "receipt_identity": ri, "offered_class": oc, "offered_identity": oi}
        clean = _check_publication_attest({**payload, "claimed_accepted": accepted})
        if clean:
            failures.append(f"attest {payload!r}: the reference's own honest claim was flagged")
        mut = _check_publication_attest({**payload, "claimed_accepted": not accepted})
        mutate(bool(mut), f"attest {payload!r} (claim inverted)")
    need("attest_substitution_cases", "an offered name that is not the receipt's own handle")

    # -- identity-outcome ----------------------------------------------------------------
    outcome_cases = [
        (b"identical", b"identical", "id-a", "id-a", "converged"),
        (b"identical-2", b"identical-2", "id-b", "id-b", "converged"),
        (b"original", b"impostor", "same-id", "same-id", "aborted"),
        (b"first", b"second", "same-id-2", "same-id-2", "aborted"),
        (b"alpha", b"beta", "id-alpha", "id-beta", "distinct"),
        (b"gamma", b"delta", "id-gamma", "id-delta", "distinct"),
    ]
    for ca, cb, ida, idb, outcome in outcome_cases:
        bump(f"outcome_{outcome}")
        payload = {
            "content_a": ca.hex(),
            "content_b": cb.hex(),
            "identity_a": ida,
            "identity_b": idb,
        }
        clean = _check_identity_outcome({**payload, "claimed_outcome": outcome})
        if clean:
            failures.append(f"identity outcome {payload!r}: the reference's own honest claim was flagged")
        for wrong in {"converged", "aborted", "distinct"} - {outcome}:
            mut = _check_identity_outcome({**payload, "claimed_outcome": wrong})
            mutate(bool(mut), f"identity outcome {payload!r} (claimed {wrong!r} instead of {outcome!r})")
    for outcome in ("converged", "aborted", "distinct"):
        need(f"outcome_{outcome}", f"an identity case whose real outcome is {outcome!r}")

    # -- digest-token ----------------------------------------------------------------------
    try:
        token_len = _extract_digest_token_len()
        bump("digest_token_len_extracted")
    except ValueError as exc:
        failures.append(str(exc))
        token_len = 64
    good = "a1" * (token_len // 2)
    token_cases = [
        (good, True),
        ("", False),
        (good[:-1], False),
        (good + "0", False),
        ("A" + good[1:], False),
        (good[:-1] + "g", False),
    ]
    for token, accepted in token_cases:
        bump("digest_token_cases")
        if not accepted:
            bump("digest_token_tamper_cases")
        clean = _check_digest_token({"token": token, "claimed_accepted": accepted})
        if clean:
            failures.append(f"digest token {token!r}: the reference's own honest claim was flagged")
        mut = _check_digest_token({"token": token, "claimed_accepted": not accepted})
        mutate(bool(mut), f"digest token {token!r} (claim inverted)")
    need("digest_token_len_extracted", "a real DIGEST_BITS constant")
    need("digest_token_tamper_cases", "a malformed or tampered digest token")

    # -- named real Rust test evidence -----------------------------------------------------
    rust_tests: list[dict[str, Any]] = []
    for path, fn_name in RUST_TESTS:
        ok, msg = _rust_test_present(path, fn_name)
        rust_tests.append({"path": str(path.relative_to(ROOT)), "test": fn_name, "present": ok})
        bump("rust_tests_named")
        if ok:
            bump("rust_tests_present")
        else:
            failures.append(f"named Rust test evidence missing: {msg}")
    need("rust_tests_present", "named real, existing, non-#[ignore]d Rust test")

    if mutants["applied"] and mutants["detected"] == 0:
        failures.append("TEST-9-07: the mutant was never detected across the corpus (the checker would be vacuous)")

    entry: dict[str, Any] = {
        "status": STATUS["TEST-9-07"],
        "rules": sorted(RULES),
        "failures": failures,
        "corpus": corpus,
        "mutant": mutants,
        "rust_tests": rust_tests,
        "boundaries": BOUNDARIES["TEST-9-07"],
        "absence": ABSENCE["TEST-9-07"],
    }
    return {"requirements": {"TEST-9-07": entry}}
