//! Dedicated exit evidence for `PR-2-EXIT` (`notes/plan/notes/PLAN_REQUIREMENTS.json`,
//! id `PR-2-EXIT`; `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 2's Exit line).
//!
//! > **Exit:** concurrent publication of identical artifacts yields one identity;
//! > artificial hash collisions are detected and resolved by canonical comparison in
//! > certified lanes.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 2
//!
//! This file asserts that sentence end to end, as directly as it reads, the way
//! `crates/continuum-task/src/result.rs` did for PR 1's exit (bn-1w1y). It is new
//! machinery only where the sentence needs machinery the existing suites do not already
//! provide as a single, named, standalone witness; everywhere else it deliberately
//! reruns the established idioms of
//! [`dx13_falsification.rs`](dx13_falsification.rs) — a total-collision
//! [`ContentIdentifier`], a barrier-synchronized multi-writer race, and the
//! pause-first-publisher-at-a-phase rendezvous — duplicated locally rather than
//! imported, so this file compiles and reads on its own.
//!
//! # Evidence map
//!
//! The exit sentence has three clauses. Each is carried end to end by one test in this
//! file; the pre-existing suites named beside it reached the same claim first, under
//! heavier and more adversarial load, and remain the corroborating evidence:
//!
//! - **"concurrent publication of identical artifacts yields one identity"** —
//!   [`positive_concurrent_publication_of_byte_identical_artifacts_yields_one_identity`].
//!   Corroborated by
//!   `continuum_workspace::publication::tests::concurrent_identical_publication_yields_one_identity_and_every_receipt`
//!   (16 writers × 8 rounds, `src/publication.rs`, the G0-DX-13 reference test) and by
//!   `dx13_falsification::identical_publication_under_doubled_contention_keeps_one_identity`
//!   and
//!   `dx13_falsification::mixed_identical_and_distinct_content_stays_partitioned_under_contention`
//!   (the falsification campaign, bn-21dd).
//! - **"artificial hash collisions are detected … resolved by canonical comparison"**
//!   (the negative/boundary case: byte-*different* payloads collided on purpose) —
//!   [`negative_artificial_hash_collision_between_distinct_payloads_is_refused_not_conflated`].
//!   Corroborated by
//!   `continuum_workspace::publication::tests::an_identity_collision_aborts_rather_than_conflating_two_artifacts`
//!   (`src/publication.rs`, the store-side exact-comparison unit test) and by
//!   `dx13_falsification::concurrent_publication_of_colliding_distinct_values_never_conflates_them`
//!   and `dx13_falsification::a_total_hash_collision_under_contention_admits_exactly_one_artifact`
//!   (the same claim under heavier contention). The identity seam attacked here
//!   ([`ContentIdentifier`]) is `continuum-workspace`'s; the hash seam one layer down
//!   (`continuum-value`'s `ContentHasher`) carries the identical claim under real
//!   adversarial hashers in `continuum_value::identity::tests::certified_identity_equality_cannot_consult_a_hash`,
//!   `continuum_value::identity::tests::an_index_under_a_maximally_colliding_hash_still_separates_every_value`,
//!   and `continuum_value::identity::tests::equal_values_get_one_identity_even_when_everything_collides`
//!   — and `dx13_mutation_campaign::mutation_overwrite_on_collision_is_caught_by_the_exact_comparison_check`
//!   shows the comparison is load-bearing: a mutant that skips it is caught by nothing
//!   else.
//! - **boundary: the same collision under GC pressure** —
//!   [`boundary_artificial_collision_under_gc_pressure_leaves_the_refusal_unchanged`].
//!   This is the bn-2siid regression surface: garbage collection racing a publication
//!   that has committed content but not yet its index entry used to delete the very
//!   content ADR-0013's exact comparison needed, letting a collision through
//!   undetected. Corroborated by
//!   `dx13_falsification::concurrent_gc_must_not_let_a_receipt_name_another_publishers_bytes`
//!   (the sharper, deterministic reproduction the fix was written against),
//!   `dx13_falsification::a_receipt_must_name_a_readable_artifact_when_gc_runs_concurrently`,
//!   and `continuum_workspace::publication::tests::garbage_collection_never_reclaims_a_publication_still_in_flight`
//!   (`src/publication.rs`, the single-threaded statement of the same pin).
//!
//! # House rules, inherited from the falsification campaign
//!
//! - **`src/` is not touched.** Nothing here edits, weakens, or moves any existing test
//!   or source file.
//! - **Assertions are on outcomes, not on interleavings.** The GC-pressure test forces
//!   one interleaving through a fault seam so the race is deterministic to run; it
//!   asserts what the store holds afterward, never that the forced interleaving
//!   occurred.
//! - **Determinism is a repeated-round invariant**, not a same-order assumption:
//!   [`summarize`] sorts every multiset before two rounds are compared, because receipt
//!   *arrival* order is genuinely nondeterministic (plan §18.5's append-only ledger)
//!   while what the store *holds* is not.

use std::collections::BTreeSet;
use std::sync::{Arc, Barrier, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread;

use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{
    AbortReason, ActorId, AuditLog, AuthorityLevel, CapabilityDescriptor, CapabilityToken,
    ContentIdentifier, IdentityUnavailable, PublicationPhase, PublicationReceipt, PublishRefusal,
    ReferenceStore, StorageFaults, StoreAudit,
};

/// The artifact class every payload in this file is published under.
const CLASS: ArtifactClass = ArtifactClass::Evidence;

// --- identity seams ------------------------------------------------------------------

/// A total, deterministic content identifier: distinct bytes get distinct identities.
///
/// Identity is `continuum-value`'s decision (ADR-0013); this is the smallest pure
/// function that lets the *store's* convergence obligation be exercised without
/// importing one, duplicated from `dx13_falsification.rs`'s `Fnv1aIdentifier`.
/// `wrapping_mul` is deliberate: `overflow-checks` is on in release (`Cargo.toml`).
#[derive(Debug, Clone, Copy)]
struct Fnv1aIdentifier;

impl ContentIdentifier for Fnv1aIdentifier {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in content {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        ArtifactHandle::new(class, &format!("{hash:016x}")).map_err(|_| IdentityUnavailable)
    }
}

/// An artificial hash collision, injected at the identity seam this module takes as
/// opaque: every payload in one class maps to the same identity, whatever its bytes.
///
/// This is "artificial hash collisions" from the exit sentence, stood up without a real
/// hash function — the worst `ContentIdentifier` that still type-checks, matching
/// `dx13_falsification.rs`'s `TotalColliderIdentifier`.
#[derive(Debug, Clone, Copy)]
struct CollidingIdentifier;

impl ContentIdentifier for CollidingIdentifier {
    fn identify(
        &self,
        class: ArtifactClass,
        _content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        ArtifactHandle::new(class, "collision").map_err(|_| IdentityUnavailable)
    }
}

// --- fault seam ------------------------------------------------------------------------

/// Storage that stops the **first** publication to reach `phase` and holds it there
/// until the test releases it.
///
/// The interleaving forcer duplicated from `dx13_falsification.rs`'s `PauseFirstAt`.
/// `StorageFaults::check` runs *outside* the store's lock at every phase, so a
/// publication parked here is genuinely mid-flight: its content commit has landed and
/// its index commit has not, and every other store operation — including
/// `collect_garbage` — remains available to other threads while it waits.
#[derive(Debug)]
struct PauseFirstAt {
    phase: PublicationPhase,
    reached: Mutex<Option<mpsc::Sender<()>>>,
    resume: Mutex<Option<mpsc::Receiver<()>>>,
}

impl PauseFirstAt {
    /// Build the rendezvous plus the two ends the test drives it with: a receiver that
    /// fires once a publication has parked, and a sender that releases it.
    fn new(phase: PublicationPhase) -> (Self, mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (reached_tx, reached_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        (
            Self {
                phase,
                reached: Mutex::new(Some(reached_tx)),
                resume: Mutex::new(Some(resume_rx)),
            },
            reached_rx,
            resume_tx,
        )
    }
}

impl StorageFaults for PauseFirstAt {
    fn check(&self, phase: PublicationPhase) -> Result<(), AbortReason> {
        if phase != self.phase {
            return Ok(());
        }
        let notify = lock(&self.reached).take();
        let waiter = lock(&self.resume).take();
        if let Some(sender) = notify {
            let _ = sender.send(());
        }
        if let Some(receiver) = waiter {
            let _ = receiver.recv();
        }
        Ok(())
    }
}

/// Storage that never fails, for the tests that carry no fault-injection claim.
#[derive(Debug, Clone, Copy)]
struct NoFaultsHere;

impl StorageFaults for NoFaultsHere {
    fn check(&self, _phase: PublicationPhase) -> Result<(), AbortReason> {
        Ok(())
    }
}

/// Lock, tolerating poisoning, exactly as the store under test does.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// --- fixtures ----------------------------------------------------------------------------

/// Mint a capability token from a test-chosen identity (entropy is a capability,
/// INV-005).
fn token(identity: &str) -> CapabilityToken {
    CapabilityToken::mint(identity).expect("test capability identity is well formed")
}

/// Describe a capability at `level` for `actor`.
fn grant(identity: &str, actor: &str, level: AuthorityLevel) -> CapabilityDescriptor {
    CapabilityDescriptor::new(token(identity), ActorId::new(actor), level)
}

/// The token writer `index` publishes under.
fn publisher_token(index: usize) -> CapabilityToken {
    token(&format!("k{index}"))
}

/// A store with `writers` distinct `propose` capabilities, one `read`, and one
/// `promote`.
///
/// Distinct capabilities per thread are load-bearing for "no lost receipts": with one
/// shared token, N receipts and one receipt cloned N times are indistinguishable.
fn store_with(
    identifier: impl ContentIdentifier + 'static,
    faults: impl StorageFaults + 'static,
    writers: usize,
) -> Arc<ReferenceStore> {
    let mut builder = ReferenceStore::builder(identifier, Arc::new(AuditLog::new()))
        .faults(faults)
        .capability(grant("operator", "operator", AuthorityLevel::Promote))
        .capability(grant("reader", "reader", AuthorityLevel::Read));
    for index in 0..writers {
        builder = builder.capability(grant(
            &format!("k{index}"),
            &format!("writer{index}"),
            AuthorityLevel::Propose,
        ));
    }
    Arc::new(builder.build())
}

/// A schedule-independent description of one identity's standing in a finished store.
///
/// Receipt *arrival order* is genuinely nondeterministic (the ledger is an append-only
/// log, plan §18.5); the receipt *multiset* is not, so it is sorted here. What must be
/// deterministic is what the store holds, not the order threads reached it.
fn summarize(view: &StoreAudit<'_>, identity: &ArtifactHandle) -> String {
    let mut receipts = view.receipts(identity);
    receipts.sort();
    format!(
        "identities={:?} attribution={:?} defects={:?} aborts={:?} receipts={:?}",
        view.identities(),
        view.storage_attribution(),
        view.fsck(&Fnv1aIdentifier),
        view.aborts(),
        receipts
    )
}

// --- PR-2-EXIT, clause 1: convergence ---------------------------------------------------

/// **PR-2-EXIT, positive.** "Concurrent publication of identical artifacts yields one
/// identity": many writers, many rounds, one payload.
///
/// Every writer publishes the exact same bytes, released together by a [`Barrier`] to
/// maximize contention. The assertions hold whatever order the threads actually ran in:
/// one identity, every writer receipted, and a store state that does not vary across
/// rounds — the three clauses of the G0-DX-13 pass condition this exit line restates
/// (`notes/plan/notes/G0_SPIKE_MATRIX.md`).
#[test]
fn positive_concurrent_publication_of_byte_identical_artifacts_yields_one_identity() {
    const WRITERS: usize = 10;
    const ROUNDS: usize = 5;
    const PAYLOAD: &[u8] = b"pr-2 exit: byte-identical artifact";

    let mut summaries = Vec::new();

    for _round in 0..ROUNDS {
        let store = store_with(Fnv1aIdentifier, NoFaultsHere, WRITERS);
        let barrier = Arc::new(Barrier::new(WRITERS));

        let workers: Vec<_> = (0..WRITERS)
            .map(|index| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                let capability = publisher_token(index);
                thread::spawn(move || {
                    barrier.wait();
                    store.publish(CLASS, PAYLOAD.to_vec(), &capability)
                })
            })
            .collect();

        let receipts: Vec<PublicationReceipt> = workers
            .into_iter()
            .map(|worker| worker.join().expect("no writer panicked"))
            .map(|result| result.expect("every writer of identical bytes publishes"))
            .collect();

        // One identity: every writer's receipt names the same handle.
        let identities: BTreeSet<ArtifactHandle> =
            receipts.iter().map(|r| r.handle().clone()).collect();
        assert_eq!(
            identities.len(),
            1,
            "concurrent writers of identical bytes disagreed on identity"
        );
        let identity = receipts[0].handle().clone();

        let operator = token("operator");
        let view = store.audit_view(&operator).expect("operator");
        assert_eq!(
            view.published_count(),
            1,
            "more than one identity was published for identical bytes"
        );

        // Every writer receipted: N writers, N distinct receipted actors.
        let ledger = view.receipts(&identity);
        assert_eq!(ledger.len(), WRITERS, "a writer's receipt was lost");
        let actors: BTreeSet<&ActorId> = ledger.iter().map(PublicationReceipt::actor).collect();
        assert_eq!(
            actors.len(),
            WRITERS,
            "two writers were credited with the same receipt"
        );

        // The one identity reads back exactly the bytes every writer sent.
        assert_eq!(
            store.read(&identity, &token("reader")).expect("published"),
            PAYLOAD
        );
        assert_eq!(view.fsck(&Fnv1aIdentifier), Vec::new());
        assert_eq!(view.aborts(), Vec::new());

        summaries.push(summarize(&view, &identity));
    }

    // Deterministic result: the store state after convergence is a function of the
    // inputs, not of the schedule.
    assert!(
        summaries.windows(2).all(|pair| pair[0] == pair[1]),
        "store state after convergence varied across rounds: {summaries:#?}"
    );
}

// --- PR-2-EXIT, clause 2: artificial collision, detected and resolved -------------------

/// **PR-2-EXIT, negative/boundary.** "Artificial hash collisions are detected and
/// resolved by canonical comparison": many writers, three byte-*different* payloads, one
/// artificial identity.
///
/// [`CollidingIdentifier`] maps every payload in `CLASS` onto the same handle, so every
/// writer of bytes other than the first-committed payload collides by construction. The
/// three outcomes this asserts are the whole of "detected and resolved by canonical
/// comparison": every writer of the winning bytes is receipted for them; every writer of
/// different bytes is refused with `AbortReason::IdentityCollision` — never silently
/// merged into the winner's identity and never handed a receipt for bytes it did not
/// write; and the winning identity, read back after every collision has been resolved,
/// is still exactly the winner's own bytes.
#[test]
fn negative_artificial_hash_collision_between_distinct_payloads_is_refused_not_conflated() {
    const WRITERS: usize = 24;
    let payloads = [
        b"original".to_vec(),
        b"impostor".to_vec(),
        b"decoy".to_vec(),
    ];

    let store = store_with(CollidingIdentifier, NoFaultsHere, WRITERS);
    let barrier = Arc::new(Barrier::new(WRITERS));

    let workers: Vec<_> = (0..WRITERS)
        .map(|index| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            let capability = publisher_token(index);
            let payload = payloads[index % payloads.len()].clone();
            thread::spawn(move || {
                barrier.wait();
                let result = store.publish(CLASS, payload.clone(), &capability);
                (payload, result)
            })
        })
        .collect();

    let outcomes: Vec<(Vec<u8>, Result<PublicationReceipt, PublishRefusal>)> = workers
        .into_iter()
        .map(|worker| worker.join().expect("no writer panicked"))
        .collect();

    let operator = token("operator");
    let view = store.audit_view(&operator).expect("operator");

    // Detected: the artificial collision still produces exactly one identity, not one
    // per payload.
    assert_eq!(
        view.published_count(),
        1,
        "the artificial collision produced more than one identity"
    );
    let identity = view.identities().first().cloned().expect("one identity");
    let stored = store
        .read(&identity, &token("reader"))
        .expect("the winner is published");
    assert!(
        payloads.contains(&stored),
        "the published bytes are not any writer's payload: {stored:?}"
    );

    // Resolved by canonical comparison: winners of the exact winning bytes succeed and
    // are receipted for them; everyone else is refused with IdentityCollision, never
    // conflated and never handed the winner's identity for bytes it did not write.
    let mut winners = 0usize;
    for (payload, result) in &outcomes {
        if *payload == stored {
            let receipt = result
                .as_ref()
                .unwrap_or_else(|e| panic!("a writer of the winning bytes was refused: {e}"));
            assert_eq!(receipt.handle(), &identity);
            winners += 1;
        } else {
            match result {
                Ok(receipt) => panic!(
                    "writer of {payload:?} was handed a receipt ({}) for bytes it never \
                     wrote: canonical comparison was bypassed",
                    receipt.handle()
                ),
                Err(PublishRefusal::Aborted(aborted)) => {
                    assert_eq!(
                        aborted.reason(),
                        AbortReason::IdentityCollision,
                        "a colliding writer was refused for a reason other than the collision"
                    );
                }
                Err(PublishRefusal::CapabilityDenied(_)) => {
                    panic!("an artificial collision was reported as a capability denial")
                }
            }
        }
    }
    assert!(winners > 0, "nobody published the winning bytes");
    assert_eq!(
        view.receipts(&identity).len(),
        winners,
        "the receipt count does not match the number of writers of the winning bytes"
    );

    // The winning identity still reads back its own bytes once every collision in the
    // round has been resolved — not a loser's bytes, not a mix of the two.
    assert_eq!(
        store.read(&identity, &token("reader")).expect("published"),
        stored,
        "the winning identity's bytes changed after resolving the surrounding collisions"
    );
    assert_eq!(view.fsck(&CollidingIdentifier), Vec::new());
}

// --- PR-2-EXIT, boundary: the same collision under GC pressure --------------------------

/// **PR-2-EXIT, boundary (bn-2siid).** The same detection-and-resolution claim as
/// [`negative_artificial_hash_collision_between_distinct_payloads_is_refused_not_conflated`],
/// now raced against [`ReferenceStore::collect_garbage`] while the first writer is
/// parked between its content commit and its index commit.
///
/// Before bn-2siid, a root set computed from the index alone called that window
/// unreachable, so garbage collection deleted the very content ADR-0013's exact
/// comparison needed to detect the second writer's collision — the collision went
/// undetected, and the second writer's bytes were published under the first writer's
/// identity
/// (`dx13_falsification::concurrent_gc_must_not_let_a_receipt_name_another_publishers_bytes`).
/// The fix pins a publication's identity as a collection root from its content commit
/// until its index commit, so this test's outcome must be identical to the unraced
/// negative test above: GC pressure changes nothing about what is detected or how it is
/// resolved.
#[test]
fn boundary_artificial_collision_under_gc_pressure_leaves_the_refusal_unchanged() {
    let (faults, reached, resume) = PauseFirstAt::new(PublicationPhase::CommittingIndex);
    let store = store_with(CollidingIdentifier, faults, 2);

    let first = {
        let store = Arc::clone(&store);
        thread::spawn(move || {
            let capability = publisher_token(0);
            store.publish(CLASS, b"original".to_vec(), &capability)
        })
    };

    reached
        .recv()
        .expect("the first writer parked between its two commits");

    // GC pressure: collect while the first publication has committed its content and
    // not yet its index entry.
    let reclaimed = store
        .collect_garbage(&token("operator"))
        .expect("operator may collect");
    assert_eq!(
        reclaimed,
        Vec::new(),
        "GC reclaimed content a live publication had already been told was durable \
         (the bn-2siid pin should have prevented this)"
    );

    // The second, byte-different writer races the collision immediately after GC ran,
    // while the first writer is still parked.
    let second = store.publish(CLASS, b"impostor".to_vec(), &publisher_token(1));

    resume.send(()).expect("release the first writer");
    let first = first
        .join()
        .expect("no writer panicked")
        .expect("the first publication still succeeds under GC pressure");

    // Detected and resolved exactly as without GC pressure.
    match second {
        Ok(receipt) => panic!(
            "the second writer was handed a receipt ({}) instead of a collision refusal: \
             GC pressure let it bypass canonical comparison",
            receipt.handle()
        ),
        Err(PublishRefusal::Aborted(aborted)) => {
            assert_eq!(
                aborted.reason(),
                AbortReason::IdentityCollision,
                "the second writer was refused for a reason other than the collision"
            );
        }
        Err(PublishRefusal::CapabilityDenied(_)) => {
            panic!("an artificial collision under GC pressure was reported as a capability denial")
        }
    }

    // The winning identity still reads back its own bytes.
    assert_eq!(
        store.read(first.handle(), &token("reader")),
        Ok(b"original".to_vec()),
        "the first writer's receipt names bytes it never wrote"
    );
    let view = store.audit_view(&token("operator")).expect("operator");
    assert_eq!(view.fsck(&CollidingIdentifier), Vec::new());
    assert_eq!(view.receipts(first.handle()).len(), 1);
}
