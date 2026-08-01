//! G0-DX-13 falsification campaign: *does content addressing survive concurrency?*
//!
//! # Why this file exists
//!
//! `notes/plan/notes/G0_SPIKE_MATRIX.md` states the G0-DX-13 pass condition as
//! **"one semantic identity; no lost receipts; deterministic result"**, and its
//! promotion rule refuses to turn a spike into architecture without *adversarial
//! mutations*. The unit tests in
//! [`crates/continuum-workspace/src/publication.rs`](../src/publication.rs) were written
//! by the same hand as the implementation and assert that the condition holds. This file
//! is the other half: it exists to make the condition **false**, and it is deliberately
//! hostile to the code it exercises.
//!
//! It attacks along five axes:
//!
//! 1. **contention** — more publishers, more rounds, and a mixed workload of identical
//!    and distinct payloads in one store, so convergence and partitioning are stressed
//!    at the same time;
//! 2. **abandoned work** — staged and content-committed publications that are dropped
//!    (the modeled crash) interleaved with publications that complete;
//! 3. **fault injection at every phase** — `StorageFaults` refusing a deterministic half
//!    of the writes at `Staging`, `CommittingContent`, and `CommittingIndex` while many
//!    threads publish the same identity;
//! 4. **the collision seam** — many threads publishing byte-*different* values that an
//!    adversarial [`ContentIdentifier`] maps onto one identity, concurrently;
//! 5. **ledger races** — publication concurrent with reads, with the index verifier
//!    (`fsck`), and with garbage collection.
//!
//! # House rules for every test here
//!
//! - **Assertions are on outcomes, never on interleavings.** A test may *force* an
//!   interleaving through the [`StorageFaults`] seam — that is what the seam is for,
//!   and docs/35's acceptance list asks for exactly it — but no test asserts that a
//!   particular interleaving occurred. A test that can fail because a machine was
//!   loaded is not evidence.
//! - **Determinism is checked as a repeated-round invariant.** Receipt *arrival order*
//!   is genuinely nondeterministic (the ledger is an append-only log of who arrived
//!   when, plan §18.5); what must be schedule-independent is what the store *holds*, so
//!   [`summarize`] sorts every multiset before comparing rounds.
//! - **`src/` is not touched.** A falsification campaign that edits the thing it is
//!   falsifying proves nothing.
//!
//! # Result of the campaign
//!
//! Axes 1–4 did not break: twelve attacks, up to 48 threads, and the pass condition held
//! every time. Axis 5 did break — garbage collection racing an in-flight publication. See
//! the `violations` section at the bottom of this file: two deterministic reproductions of
//! a genuine defect, and a third corroborating it without any seam.
//!
//! Those three were retained `#[ignore]`d **because they failed**, not because they were
//! slow or flaky, so the follow-up fix would have a red test to turn green. That fix has
//! landed (bn-2siid: `ReferenceStore`'s reachability root set now includes publications
//! between their two commits, not the index alone), the `#[ignore]`s are gone, and all
//! three run in the default suite as regression guards. They must not be deleted or
//! weakened.
//!
//! The campaign's own rule — *`src/` is not touched* — held for the campaign (bn-21dd),
//! which is why the diagnosis and the fix are separate bones: the evidence that the defect
//! was real was produced without any opportunity to shape the implementation around it.
//!
//! The campaign's honest limit: this is one process, one `Mutex`, and no disk. It says
//! nothing about daemon-scale linearizability, real crash recovery, or a durable store;
//! that evidence is `continuumd`'s, PR 6+ (docs/35).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread;

use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{
    AbortReason, Action, ActorId, AuditLog, AuthorityLevel, CapabilityDenied, CapabilityDescriptor,
    CapabilityToken, ContentIdentifier, IdentityUnavailable, PublicationPhase, PublicationReceipt,
    PublishRefusal, ReferenceStore, StorageFaults, StoreAudit, StoreDefect,
};

/// The class every payload in this file is published under, unless a test says otherwise.
const CLASS: ArtifactClass = ArtifactClass::Evidence;

// --- seams -------------------------------------------------------------------------------

/// A total, deterministic content identifier.
///
/// Identity is `continuum-value`'s decision (ADR-0013); this is the smallest pure
/// function that lets the *store's* obligations be attacked without importing one.
/// `wrapping_mul` is deliberate — `overflow-checks` is on in release.
#[derive(Debug, Clone, Copy)]
struct Fnv1aIdentifier;

impl ContentIdentifier for Fnv1aIdentifier {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        ArtifactHandle::new(class, &fnv1a(content)).map_err(|_| IdentityUnavailable)
    }
}

/// The identity [`Fnv1aIdentifier`] derives, as a bare string, for tests that need to
/// know a handle before it exists.
fn fnv1a(content: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in content {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The handle [`Fnv1aIdentifier`] will derive for `content`.
fn fnv1a_handle(content: &[u8]) -> ArtifactHandle {
    ArtifactHandle::new(CLASS, &fnv1a(content)).expect("fnv identity is a legal handle identity")
}

/// An adversarial identifier that maps *every* content to one identity.
///
/// The worst hash function that still type-checks. ADR-0013 says hashes only index and
/// partition, and collisions resolve by exact comparison; this seam is how that claim is
/// attacked rather than assumed.
#[derive(Debug, Clone, Copy)]
struct TotalColliderIdentifier;

impl ContentIdentifier for TotalColliderIdentifier {
    fn identify(
        &self,
        class: ArtifactClass,
        _content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        ArtifactHandle::new(class, "collision").map_err(|_| IdentityUnavailable)
    }
}

/// An adversarial identifier that partitions content into colliding *families* by its
/// first byte.
///
/// Nastier than a total collider for a concurrency campaign: several distinct collision
/// classes are contended at once, so a store that resolves one collision correctly in
/// isolation still has to do it under load, several times over, against interleaved
/// convergence of genuinely identical payloads.
#[derive(Debug, Clone, Copy)]
struct FirstByteIdentifier;

impl ContentIdentifier for FirstByteIdentifier {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        let family = content.first().copied().unwrap_or(0);
        ArtifactHandle::new(class, &format!("family{family:02x}")).map_err(|_| IdentityUnavailable)
    }
}

/// Storage that refuses every other write at one phase.
///
/// The failure *count* is deterministic (a global counter, one increment per check at
/// the target phase), while *which* publisher is refused is up to the scheduler. That is
/// exactly the shape a falsification test wants: an outcome that can be asserted
/// exactly, reached by a schedule that cannot be predicted.
#[derive(Debug)]
struct FailEveryOtherAt {
    phase: PublicationPhase,
    calls: AtomicUsize,
}

impl FailEveryOtherAt {
    fn new(phase: PublicationPhase) -> Self {
        Self {
            phase,
            calls: AtomicUsize::new(0),
        }
    }
}

impl StorageFaults for FailEveryOtherAt {
    fn check(&self, phase: PublicationPhase) -> Result<(), AbortReason> {
        if phase != self.phase {
            return Ok(());
        }
        if self.calls.fetch_add(1, Ordering::SeqCst) % 2 == 1 {
            Err(AbortReason::StorageExhausted)
        } else {
            Ok(())
        }
    }
}

/// Storage that stops the **first** publication to reach `phase` and holds it there
/// until the test releases it.
///
/// This is the campaign's interleaving forcer. `StorageFaults::check` is called *outside*
/// the store's lock at every phase, so a publication parked here is genuinely mid-flight:
/// its content commit has landed and its index commit has not, and every other store
/// operation remains available to other threads. Later arrivals find the rendezvous spent
/// and pass straight through, so exactly one interleaving is pinned and nothing else is
/// serialized.
///
/// Forcing an interleaving is not the same as asserting one. No test below asserts that
/// this rendezvous was used; they assert what the store must hold afterwards.
#[derive(Debug)]
struct PauseFirstAt {
    phase: PublicationPhase,
    reached: Mutex<Option<mpsc::Sender<()>>>,
    resume: Mutex<Option<mpsc::Receiver<()>>>,
}

impl PauseFirstAt {
    /// Build the rendezvous plus the two ends the test drives it with: a receiver that
    /// fires when a publication has parked, and a sender that releases it.
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

/// Lock, tolerating poisoning, exactly as the store under test does.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// --- fixtures ----------------------------------------------------------------------------

/// Mint a capability token from a test-chosen identity (entropy is a capability, INV-005).
fn token(identity: &str) -> CapabilityToken {
    CapabilityToken::mint(identity).expect("test capability identity is well formed")
}

/// Describe a capability at `level` for `actor`.
fn grant(identity: &str, actor: &str, level: AuthorityLevel) -> CapabilityDescriptor {
    CapabilityDescriptor::new(token(identity), ActorId::new(actor), level)
}

/// The token thread `index` publishes under.
fn publisher_token(index: usize) -> CapabilityToken {
    token(&format!("k{index}"))
}

/// A store with `publishers` distinct `propose` capabilities, one `read`, and one
/// `promote`.
///
/// Distinct capabilities per thread are load-bearing for the "no lost receipts" half of
/// the pass condition: with one shared token, N receipts and 1 receipt cloned N times are
/// indistinguishable.
fn store_with(
    identifier: impl ContentIdentifier + 'static,
    faults: impl StorageFaults + 'static,
    publishers: usize,
) -> Arc<ReferenceStore> {
    let mut builder = ReferenceStore::builder(identifier, Arc::new(AuditLog::new()))
        .faults(faults)
        .capability(grant("operator", "operator", AuthorityLevel::Promote))
        .capability(grant("reader", "reader", AuthorityLevel::Read));
    for index in 0..publishers {
        builder = builder.capability(grant(
            &format!("k{index}"),
            &format!("publisher{index}"),
            AuthorityLevel::Propose,
        ));
    }
    Arc::new(builder.build())
}

/// A schedule-independent description of a finished store.
///
/// Every multiset is sorted. What must be deterministic is what the store holds — not the
/// order threads reached it, which the ledger records precisely because it is not.
fn summarize(view: &StoreAudit<'_>) -> String {
    summarize_with(view, PublisherNames::Recorded)
}

/// The same description with publisher names elided.
///
/// Used only where an injected fault selects its victims by *arrival order*: which
/// publisher a "refuse every other write" seam refuses is a fact about the schedule, and
/// asserting it would be exactly the interleaving assertion this campaign forbids. What
/// must still be schedule-independent — the identities held, the bytes attributed, the
/// defects found, the abort multiset, and the number and cost of the receipts — is all
/// still compared.
fn summarize_without_publisher_names(view: &StoreAudit<'_>) -> String {
    summarize_with(view, PublisherNames::Elided)
}

/// Whether [`summarize_with`] records which publisher earned each receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PublisherNames {
    /// Keep the actor: the correct default, and the only way "no lost receipts" is
    /// distinguishable from "one receipt, cloned".
    Recorded,
    /// Elide the actor, keeping the receipt count and cost.
    Elided,
}

fn summarize_with(view: &StoreAudit<'_>, names: PublisherNames) -> String {
    let mut identities = view.identities();
    identities.sort();

    let mut receipts: Vec<String> = Vec::new();
    for handle in &identities {
        for receipt in view.receipts(handle) {
            let actor = match names {
                PublisherNames::Recorded => receipt.actor().to_string(),
                PublisherNames::Elided => "*".to_owned(),
            };
            receipts.push(format!(
                "{handle}|{actor}|{}|{}",
                receipt.cost().content_bytes(),
                receipt.cost().index_entries()
            ));
        }
    }
    receipts.sort();

    let mut aborts: Vec<String> = view
        .aborts()
        .iter()
        .map(|abort| format!("{}/{}", abort.phase(), abort.reason()))
        .collect();
    aborts.sort();

    format!(
        "identities={identities:?} attribution={:?} defects={:?} aborts={aborts:?} receipts={receipts:?}",
        view.storage_attribution(),
        view.fsck(),
    )
}

/// The pass condition's first two clauses, checked against a finished store.
///
/// `expected` maps each payload that should be published to the number of publishers that
/// succeeded with it. Asserts: one identity per distinct payload and no more; a receipt
/// per success and no fewer; every receipt naming an artifact that reads back to exactly
/// the bytes its publisher supplied; and no defect of any kind.
fn assert_one_identity_and_every_receipt(
    store: &ReferenceStore,
    expected: &BTreeMap<Vec<u8>, usize>,
    identify: impl Fn(&[u8]) -> ArtifactHandle,
) {
    let operator = token("operator");
    let reader = token("reader");
    let view = store.audit_view(&operator).expect("operator may audit");

    let mut wanted: BTreeMap<ArtifactHandle, (Vec<u8>, usize)> = BTreeMap::new();
    for (payload, count) in expected {
        wanted.insert(identify(payload), (payload.clone(), *count));
    }

    assert_eq!(
        view.published_count(),
        wanted.len(),
        "store holds a different number of identities than the payloads justify"
    );
    assert_eq!(
        view.identities().into_iter().collect::<BTreeSet<_>>(),
        wanted.keys().cloned().collect::<BTreeSet<_>>(),
        "published identities are not the identities the payloads derive"
    );

    for (handle, (payload, count)) in &wanted {
        assert_eq!(
            view.receipts(handle).len(),
            *count,
            "receipts lost for {handle}: the ledger is append-only (plan §18.5)"
        );
        assert_eq!(
            &store
                .read(handle, &reader)
                .expect("published artifacts read"),
            payload,
            "{handle} does not read back the bytes it was published from"
        );
        let actors: BTreeSet<ActorId> = view
            .receipts(handle)
            .iter()
            .map(|receipt| receipt.actor().clone())
            .collect();
        assert_eq!(
            actors.len(),
            *count,
            "two receipts for {handle} name the same publisher: a receipt was overwritten"
        );
    }

    assert_eq!(view.fsck(), Vec::new(), "index verifier found defects");
}

// --- axis 1: contention ------------------------------------------------------------------

/// Twice the publishers and half again the rounds of the landed exit test, on the same
/// question: N publishers of one payload, one identity, N receipts, and a store whose
/// contents do not depend on the schedule.
#[test]
fn identical_publication_under_doubled_contention_keeps_one_identity() {
    const PUBLISHERS: usize = 32;
    const ROUNDS: usize = 12;
    let payload = b"identical bytes under contention".to_vec();

    let mut summaries = Vec::new();
    for _round in 0..ROUNDS {
        let store = store_with(Fnv1aIdentifier, NoFaultsHere, PUBLISHERS);
        let barrier = Arc::new(Barrier::new(PUBLISHERS));

        let workers: Vec<_> = (0..PUBLISHERS)
            .map(|index| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                let payload = payload.clone();
                thread::spawn(move || {
                    let capability = publisher_token(index);
                    barrier.wait();
                    store.publish(CLASS, payload, &capability)
                })
            })
            .collect();

        for worker in workers {
            worker
                .join()
                .expect("no publisher panicked")
                .expect("every publication succeeds");
        }

        let expected = BTreeMap::from([(payload.clone(), PUBLISHERS)]);
        assert_one_identity_and_every_receipt(&store, &expected, fnv1a_handle);

        let view = store.audit_view(&token("operator")).expect("operator");
        assert_eq!(view.aborts(), Vec::new(), "a clean run aborted nothing");
        summaries.push(summarize(&view));
    }

    assert_deterministic(&summaries);
}

/// Identical and distinct content in one store, at once.
///
/// The landed tests check convergence and partitioning in separate stores. Doing both in
/// one store at once is the harder question: a store that keyed dedup on anything but the
/// derived identity — an insertion counter, a per-thread cache, a last-writer-wins index —
/// would answer one of the two correctly and the other one wrong.
#[test]
fn mixed_identical_and_distinct_content_stays_partitioned_under_contention() {
    const PUBLISHERS: usize = 48;
    const PAYLOADS: usize = 6;
    const ROUNDS: usize = 6;

    let payloads: Vec<Vec<u8>> = (0..PAYLOADS)
        .map(|n| format!("payload-{n}-{}", "x".repeat(n * 3)).into_bytes())
        .collect();

    let mut summaries = Vec::new();
    for _round in 0..ROUNDS {
        let store = store_with(Fnv1aIdentifier, NoFaultsHere, PUBLISHERS);
        let barrier = Arc::new(Barrier::new(PUBLISHERS));

        let workers: Vec<_> = (0..PUBLISHERS)
            .map(|index| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                let payload = payloads[index % PAYLOADS].clone();
                thread::spawn(move || {
                    let capability = publisher_token(index);
                    barrier.wait();
                    store.publish(CLASS, payload, &capability)
                })
            })
            .collect();

        for worker in workers {
            worker
                .join()
                .expect("no publisher panicked")
                .expect("every publication succeeds");
        }

        let expected: BTreeMap<Vec<u8>, usize> = payloads
            .iter()
            .cloned()
            .map(|payload| (payload, PUBLISHERS / PAYLOADS))
            .collect();
        assert_one_identity_and_every_receipt(&store, &expected, fnv1a_handle);

        let view = store.audit_view(&token("operator")).expect("operator");
        assert_eq!(
            view.storage_attribution(),
            BTreeMap::from([(CLASS, payloads.iter().map(|p| p.len() as u64).sum::<u64>())]),
            "converged content must be stored exactly once per identity"
        );
        summaries.push(summarize(&view));
    }

    assert_deterministic(&summaries);
}

/// Publication of the same bytes under different artifact classes must not converge.
///
/// Identity is derived from `(class, content)`, so this is the boundary case where a
/// store that keyed its index on content alone would silently merge two artifacts that
/// plan §4.4 keeps apart — under concurrency, where the merge would look like ordinary
/// dedup.
#[test]
fn identical_bytes_in_different_classes_never_converge_under_contention() {
    const PUBLISHERS: usize = 24;
    let classes = [
        ArtifactClass::Evidence,
        ArtifactClass::ProofArtifact,
        ArtifactClass::Crashpack,
    ];
    let payload = b"the very same bytes".to_vec();

    let store = store_with(Fnv1aIdentifier, NoFaultsHere, PUBLISHERS);
    let barrier = Arc::new(Barrier::new(PUBLISHERS));
    let workers: Vec<_> = (0..PUBLISHERS)
        .map(|index| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            let payload = payload.clone();
            let class = classes[index % classes.len()];
            thread::spawn(move || {
                let capability = publisher_token(index);
                barrier.wait();
                store.publish(class, payload, &capability)
            })
        })
        .collect();

    let handles: BTreeSet<ArtifactHandle> = workers
        .into_iter()
        .map(|worker| {
            worker
                .join()
                .expect("no publisher panicked")
                .expect("published")
                .handle()
                .clone()
        })
        .collect();

    assert_eq!(handles.len(), classes.len(), "classes were conflated");
    let view = store.audit_view(&token("operator")).expect("operator");
    assert_eq!(view.published_count(), classes.len());
    assert_eq!(
        view.storage_attribution(),
        classes
            .iter()
            .map(|class| (*class, payload.len() as u64))
            .collect::<BTreeMap<_, _>>(),
        "attribution must stay per class (plan §4.5)"
    );
    assert_eq!(view.fsck(), Vec::new());
}

// --- axis 2: abandoned work interleaved with commits -------------------------------------

/// Half the threads publish; half stage and abandon; all use the same payload.
///
/// An abandoned publication must leave *nothing* — no content, no index entry, no
/// receipt — even while other threads are converging onto the identity it named.
#[test]
fn abandoned_stagings_interleaved_with_commits_leave_no_trace() {
    const THREADS: usize = 48;
    const ROUNDS: usize = 4;
    let payload = b"abandoned or committed".to_vec();

    let mut summaries = Vec::new();
    for _round in 0..ROUNDS {
        let store = store_with(Fnv1aIdentifier, NoFaultsHere, THREADS);
        let barrier = Arc::new(Barrier::new(THREADS));

        let workers: Vec<_> = (0..THREADS)
            .map(|index| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                let payload = payload.clone();
                thread::spawn(move || {
                    let capability = publisher_token(index);
                    barrier.wait();
                    if index % 2 == 0 {
                        store.publish(CLASS, payload, &capability).map(|_| ())
                    } else {
                        let staged = store
                            .stage(CLASS, payload, &capability)
                            .expect("staging is authorized");
                        drop(staged);
                        Ok(())
                    }
                })
            })
            .collect();

        for worker in workers {
            worker
                .join()
                .expect("no thread panicked")
                .expect("no thread was refused");
        }

        let expected = BTreeMap::from([(payload.clone(), THREADS / 2)]);
        assert_one_identity_and_every_receipt(&store, &expected, fnv1a_handle);

        let view = store.audit_view(&token("operator")).expect("operator");
        let aborts = view.aborts();
        assert_eq!(aborts.len(), THREADS / 2, "one abort per abandoned staging");
        assert!(
            aborts
                .iter()
                .all(|abort| abort.phase() == PublicationPhase::Staging
                    && abort.reason() == AbortReason::Abandoned),
            "an abandoned staging aborted somewhere else: {aborts:?}"
        );
        summaries.push(summarize(&view));
    }

    assert_deterministic(&summaries);
}

/// Half the threads publish; half commit content and then crash before the index.
///
/// This is docs/35's crash residue arriving *concurrently* with a live publication of the
/// same identity. The residue must be absorbed — the content the crashed publishers wrote
/// is the same content the successful ones converged onto, so once any index entry exists
/// it is reachable and `fsck` is clean — and the crashed publishers must still get no
/// receipt.
#[test]
fn crashes_between_the_commits_are_absorbed_by_a_concurrent_successful_publication() {
    const THREADS: usize = 48;
    const ROUNDS: usize = 4;
    let payload = b"committed then crashed".to_vec();

    let mut summaries = Vec::new();
    for _round in 0..ROUNDS {
        let store = store_with(Fnv1aIdentifier, NoFaultsHere, THREADS);
        let barrier = Arc::new(Barrier::new(THREADS));

        let workers: Vec<_> = (0..THREADS)
            .map(|index| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                let payload = payload.clone();
                thread::spawn(move || {
                    let capability = publisher_token(index);
                    barrier.wait();
                    if index % 2 == 0 {
                        store.publish(CLASS, payload, &capability).map(|_| ())
                    } else {
                        let committed = store
                            .stage(CLASS, payload, &capability)
                            .expect("staging is authorized")
                            .commit_content()
                            .expect("content commits");
                        drop(committed); // the modeled crash
                        Ok(())
                    }
                })
            })
            .collect();

        for worker in workers {
            worker
                .join()
                .expect("no thread panicked")
                .expect("no thread was refused");
        }

        let expected = BTreeMap::from([(payload.clone(), THREADS / 2)]);
        assert_one_identity_and_every_receipt(&store, &expected, fnv1a_handle);

        let view = store.audit_view(&token("operator")).expect("operator");
        let aborts = view.aborts();
        assert_eq!(aborts.len(), THREADS / 2);
        assert!(
            aborts
                .iter()
                .all(|abort| abort.phase() == PublicationPhase::CommittingIndex
                    && abort.reason() == AbortReason::Abandoned),
            "a crash between the commits was recorded at the wrong phase: {aborts:?}"
        );
        assert_eq!(
            view.storage_attribution(),
            BTreeMap::from([(CLASS, payload.len() as u64)]),
            "the crashed publishers' bytes must converge, not accumulate"
        );
        summaries.push(summarize(&view));
    }

    assert_deterministic(&summaries);
}

// --- axis 3: fault injection at every phase, under concurrency ---------------------------

/// docs/35's acceptance list, run concurrently: a refusal injected at every publication
/// phase while many threads publish one identity.
///
/// The exact counts are assertable because the fault seam refuses every *other* check at
/// its phase and every publication reaches that phase exactly once; which publisher is
/// refused is the scheduler's business and is never asserted.
#[test]
fn refusals_injected_at_every_phase_under_contention_never_half_publish() {
    const THREADS: usize = 24;
    const ROUNDS: usize = 3;
    let payload = b"refused at some phase".to_vec();
    let handle = fnv1a_handle(&payload);

    for phase in [
        PublicationPhase::Staging,
        PublicationPhase::CommittingContent,
        PublicationPhase::CommittingIndex,
    ] {
        let mut summaries = Vec::new();
        for _round in 0..ROUNDS {
            let store = store_with(Fnv1aIdentifier, FailEveryOtherAt::new(phase), THREADS);
            let barrier = Arc::new(Barrier::new(THREADS));

            let workers: Vec<_> = (0..THREADS)
                .map(|index| {
                    let store = Arc::clone(&store);
                    let barrier = Arc::clone(&barrier);
                    let payload = payload.clone();
                    thread::spawn(move || {
                        let capability = publisher_token(index);
                        barrier.wait();
                        store.publish(CLASS, payload, &capability)
                    })
                })
                .collect();

            let results: Vec<Result<PublicationReceipt, PublishRefusal>> = workers
                .into_iter()
                .map(|worker| worker.join().expect("no publisher panicked"))
                .collect();

            let successes = results.iter().filter(|result| result.is_ok()).count();
            let refusals: Vec<&PublishRefusal> = results
                .iter()
                .filter_map(|result| result.as_ref().err())
                .collect();

            // Deterministic in count: half the checks at the target phase are refused.
            assert_eq!(successes, THREADS / 2, "phase {phase}");
            assert_eq!(refusals.len(), THREADS / 2, "phase {phase}");
            for refusal in &refusals {
                match refusal {
                    PublishRefusal::Aborted(aborted) => {
                        assert_eq!(aborted.phase(), phase);
                        assert_eq!(aborted.reason(), AbortReason::StorageExhausted);
                    }
                    PublishRefusal::CapabilityDenied(_) => {
                        panic!("a storage refusal was reported as a capability denial")
                    }
                }
            }

            // The pass condition still holds over the publications that survived.
            let expected = BTreeMap::from([(payload.clone(), successes)]);
            assert_one_identity_and_every_receipt(&store, &expected, fnv1a_handle);

            let view = store.audit_view(&token("operator")).expect("operator");
            assert_eq!(view.aborts().len(), THREADS / 2, "phase {phase}");
            assert_eq!(
                view.storage_attribution(),
                BTreeMap::from([(CLASS, payload.len() as u64)]),
                "phase {phase}: content is stored once whatever was refused"
            );

            // Garbage collection over the finished store must not disturb a live artifact.
            let reclaimed = store
                .collect_garbage(&token("operator"))
                .expect("operator may collect");
            assert_eq!(
                reclaimed,
                Vec::new(),
                "phase {phase}: GC reclaimed content the index still names"
            );
            assert_eq!(
                store
                    .read(&handle, &token("reader"))
                    .expect("still readable"),
                payload,
                "phase {phase}: a published artifact stopped reading after GC"
            );

            summaries.push(summarize_without_publisher_names(&view));
        }
        assert_deterministic(&summaries);
    }
}

/// The same injection when *every* publication is refused at the index commit.
///
/// Nothing may become visible, every publisher must be refused, and the residue must be
/// exactly the reclaimable unreachable content docs/35 trades for never writing a stale
/// index entry.
#[test]
fn a_universally_refused_index_commit_leaves_only_reclaimable_residue() {
    const THREADS: usize = 24;
    let payload = b"nobody gets an index entry".to_vec();
    let handle = fnv1a_handle(&payload);

    let store = store_with(
        Fnv1aIdentifier,
        AlwaysFailAt(PublicationPhase::CommittingIndex),
        THREADS,
    );
    let barrier = Arc::new(Barrier::new(THREADS));
    let workers: Vec<_> = (0..THREADS)
        .map(|index| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            let payload = payload.clone();
            thread::spawn(move || {
                let capability = publisher_token(index);
                barrier.wait();
                store.publish(CLASS, payload, &capability)
            })
        })
        .collect();

    for worker in workers {
        worker
            .join()
            .expect("no publisher panicked")
            .expect_err("every index commit was refused");
    }

    let operator = token("operator");
    let view = store.audit_view(&operator).expect("operator");
    assert_eq!(view.published_count(), 0);
    assert!(
        view.receipts(&handle).is_empty(),
        "a refusal issued a receipt"
    );
    assert_eq!(
        store.read(&handle, &token("reader")),
        Err(CapabilityDenied),
        "nothing was published, so nothing reads"
    );
    assert_eq!(
        view.fsck(),
        vec![StoreDefect::UnreachableContent(handle.clone())],
        "the residue must be classified as unreachable content, not as a missing referent"
    );
    assert_eq!(view.aborts().len(), THREADS);

    let reclaimed = store.collect_garbage(&operator).expect("operator");
    assert_eq!(reclaimed, vec![handle]);
    let view = store.audit_view(&operator).expect("operator");
    assert_eq!(view.fsck(), Vec::new());
    assert_eq!(view.storage_attribution(), BTreeMap::new());
}

// --- axis 4: the collision seam, concurrently ---------------------------------------------

/// Many threads publishing byte-different values that the identifier maps onto one
/// identity, at once.
///
/// ADR-0013's store-side rule is exact comparison, and the failure mode it forbids is
/// *conflation*: two artifacts that are not equal must not become one. Under concurrency
/// the rule has a sharper form, and it is what this test asserts — **every publisher that
/// received a receipt must have published exactly the bytes the store now holds under
/// that identity**. A receipt for someone else's bytes is a lie whether or not the store
/// looks internally consistent afterwards.
///
/// Which of the colliding values wins is genuinely schedule-dependent and is not
/// asserted; that a loser is refused with `IdentityCollision`, and that no loser holds a
/// receipt, is.
#[test]
fn concurrent_publication_of_colliding_distinct_values_never_conflates_them() {
    const FAMILIES: usize = 4;
    const THREADS_PER_FAMILY: usize = 8;
    const THREADS: usize = FAMILIES * THREADS_PER_FAMILY;
    const ROUNDS: usize = 8;

    // Two byte-different payloads per family; `FirstByteIdentifier` collides them.
    let payloads: Vec<Vec<u8>> = (0..FAMILIES)
        .flat_map(|family| {
            let tag = u8::try_from(family).expect("family fits in a byte");
            [
                vec![tag, b'A', b'l', b'p', b'h', b'a'],
                vec![tag, b'B', b'e', b't', b'a'],
            ]
        })
        .collect();

    for _round in 0..ROUNDS {
        let store = store_with(FirstByteIdentifier, NoFaultsHere, THREADS);
        let barrier = Arc::new(Barrier::new(THREADS));

        let workers: Vec<_> = (0..THREADS)
            .map(|index| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                let payload = payloads[index % payloads.len()].clone();
                thread::spawn(move || {
                    let capability = publisher_token(index);
                    barrier.wait();
                    let result = store.publish(CLASS, payload.clone(), &capability);
                    (payload, result)
                })
            })
            .collect();

        let outcomes: Vec<(Vec<u8>, Result<PublicationReceipt, PublishRefusal>)> = workers
            .into_iter()
            .map(|worker| worker.join().expect("no publisher panicked"))
            .collect();

        let operator = token("operator");
        let reader = token("reader");
        let view = store.audit_view(&operator).expect("operator");
        assert_eq!(
            view.published_count(),
            FAMILIES,
            "one identity per collision family, whoever won"
        );

        let mut successes_per_identity: BTreeMap<ArtifactHandle, usize> = BTreeMap::new();
        for (payload, result) in &outcomes {
            match result {
                Ok(receipt) => {
                    let stored = store
                        .read(receipt.handle(), &reader)
                        .expect("a receipt names a published artifact");
                    assert_eq!(
                        &stored, payload,
                        "a publisher holds a receipt for bytes it did not publish"
                    );
                    *successes_per_identity
                        .entry(receipt.handle().clone())
                        .or_default() += 1;
                }
                Err(PublishRefusal::Aborted(aborted)) => {
                    assert_eq!(
                        aborted.reason(),
                        AbortReason::IdentityCollision,
                        "a colliding publication was refused for the wrong reason"
                    );
                    assert_eq!(aborted.phase(), PublicationPhase::CommittingContent);
                }
                Err(PublishRefusal::CapabilityDenied(_)) => {
                    panic!("a collision was reported as a capability denial")
                }
            }
        }

        for (handle, successes) in &successes_per_identity {
            assert_eq!(
                view.receipts(handle).len(),
                *successes,
                "receipts lost under collision contention"
            );
        }
        assert_eq!(
            successes_per_identity.len(),
            FAMILIES,
            "a family produced no winner at all"
        );
        assert_eq!(
            successes_per_identity.values().sum::<usize>()
                + outcomes.iter().filter(|(_, r)| r.is_err()).count(),
            THREADS,
            "a publication neither succeeded nor was refused"
        );
        assert_eq!(view.fsck(), Vec::new());
    }
}

/// The total-collider case: every payload in the store maps to one identity.
///
/// The degenerate end of the collision seam. Exactly one payload may be stored, exactly
/// the publishers of that payload may hold receipts, and everyone else must be refused.
#[test]
fn a_total_hash_collision_under_contention_admits_exactly_one_artifact() {
    const THREADS: usize = 32;
    const ROUNDS: usize = 6;
    let payloads = [
        b"original".to_vec(),
        b"impostor".to_vec(),
        b"third".to_vec(),
    ];

    for _round in 0..ROUNDS {
        let store = store_with(TotalColliderIdentifier, NoFaultsHere, THREADS);
        let barrier = Arc::new(Barrier::new(THREADS));

        let workers: Vec<_> = (0..THREADS)
            .map(|index| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                let payload = payloads[index % payloads.len()].clone();
                thread::spawn(move || {
                    let capability = publisher_token(index);
                    barrier.wait();
                    let result = store.publish(CLASS, payload.clone(), &capability);
                    (payload, result)
                })
            })
            .collect();

        let outcomes: Vec<(Vec<u8>, Result<PublicationReceipt, PublishRefusal>)> = workers
            .into_iter()
            .map(|worker| worker.join().expect("no publisher panicked"))
            .collect();

        let operator = token("operator");
        let view = store.audit_view(&operator).expect("operator");
        assert_eq!(view.published_count(), 1);
        let identity = view.identities().first().cloned().expect("one identity");
        let stored = store
            .read(&identity, &token("reader"))
            .expect("the winner is published");

        let winners: Vec<&Vec<u8>> = outcomes
            .iter()
            .filter(|(_, result)| result.is_ok())
            .map(|(payload, _)| payload)
            .collect();
        assert!(!winners.is_empty(), "nobody won a total collision");
        assert!(
            winners.iter().all(|payload| **payload == stored),
            "a publisher of different bytes received a receipt for the winner's identity"
        );
        assert_eq!(
            view.receipts(&identity).len(),
            winners.len(),
            "receipts lost under a total collision"
        );
        assert_eq!(
            view.aborts().len(),
            THREADS - winners.len(),
            "every loser must leave exactly one abort record"
        );
        assert_eq!(view.fsck(), Vec::new());
    }
}

// --- axis 5a: reads and the index verifier, concurrent with publication ------------------

/// Publication concurrent with reads and with the index verifier.
///
/// A concurrent `fsck` may legitimately see *unreachable content* — that is a publication
/// mid-flight, and it is the residue class docs/35 accepts. The two classes it must never
/// see are the ones that mean the store lied: a **missing referent** (an index entry with
/// no content, i.e. the index was written first) and an **identity mismatch** (content
/// filed under an identity it does not derive).
///
/// Readers assert the same rule from outside: a read either refuses or returns exactly
/// the bytes that identity was published from. Never a prefix, never another artifact.
#[test]
fn readers_and_fsck_running_against_a_publishing_store_never_see_a_half_artifact() {
    const PUBLISHERS: usize = 16;
    const READERS: usize = 8;
    const VERIFIERS: usize = 4;
    const PROBES: usize = 200;
    const ROUNDS: usize = 3;

    for _round in 0..ROUNDS {
        let payloads: Vec<Vec<u8>> = (0..PUBLISHERS)
            .map(|n| format!("concurrent payload {n}").into_bytes())
            .collect();
        let expected: BTreeMap<ArtifactHandle, Vec<u8>> = payloads
            .iter()
            .map(|payload| (fnv1a_handle(payload), payload.clone()))
            .collect();

        let store = store_with(Fnv1aIdentifier, NoFaultsHere, PUBLISHERS);
        let barrier = Arc::new(Barrier::new(PUBLISHERS + READERS + VERIFIERS));

        let mut workers: Vec<thread::JoinHandle<Vec<String>>> = Vec::new();

        for (index, payload) in payloads.iter().enumerate() {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            let payload = payload.clone();
            workers.push(thread::spawn(move || {
                let capability = publisher_token(index);
                barrier.wait();
                match store.publish(CLASS, payload, &capability) {
                    Ok(_) => Vec::new(),
                    Err(refusal) => vec![format!("publication refused: {refusal}")],
                }
            }));
        }

        for _ in 0..READERS {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            let expected = expected.clone();
            workers.push(thread::spawn(move || {
                let reader = token("reader");
                barrier.wait();
                let mut complaints = Vec::new();
                for _ in 0..PROBES {
                    for (handle, payload) in &expected {
                        match store.read(handle, &reader) {
                            Ok(bytes) if &bytes == payload => {}
                            Ok(bytes) => complaints.push(format!(
                                "{handle} read back {} bytes, expected {}",
                                bytes.len(),
                                payload.len()
                            )),
                            Err(CapabilityDenied) => {} // not yet published: permitted
                        }
                    }
                }
                complaints
            }));
        }

        for _ in 0..VERIFIERS {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || {
                let operator = token("operator");
                barrier.wait();
                let mut complaints = Vec::new();
                for _ in 0..PROBES {
                    let view = store.audit_view(&operator).expect("operator");
                    for defect in view.fsck() {
                        match defect {
                            // A publication between its two commits. Expected.
                            StoreDefect::UnreachableContent(_) => {}
                            other => complaints.push(format!("fsck saw {other}")),
                        }
                    }
                }
                complaints
            }));
        }

        let complaints: Vec<String> = workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("no thread panicked"))
            .collect();
        assert!(complaints.is_empty(), "{complaints:#?}");

        let expected_counts: BTreeMap<Vec<u8>, usize> = payloads
            .iter()
            .cloned()
            .map(|payload| (payload, 1))
            .collect();
        assert_one_identity_and_every_receipt(&store, &expected_counts, fnv1a_handle);
    }
}

// --- axis 5b: idempotent replay ----------------------------------------------------------

/// Idempotent replay under contention: the same publisher publishing the same bytes over
/// and over while everyone else does the same.
///
/// RFC 0026 makes a retry "a fresh publication, not a resumption of a partial one", and
/// plan §18.5 makes the ledger append-only. Together those fix the answer exactly: one
/// identity, one stored copy, and one receipt per *attempt* — not per publisher, and not
/// one collapsed receipt per identity.
#[test]
fn idempotent_replay_under_contention_appends_a_receipt_per_attempt() {
    const PUBLISHERS: usize = 16;
    const REPLAYS: usize = 8;
    const ROUNDS: usize = 3;
    let payload = b"replayed many times".to_vec();

    let mut summaries = Vec::new();
    for _round in 0..ROUNDS {
        let store = store_with(Fnv1aIdentifier, NoFaultsHere, PUBLISHERS);
        let barrier = Arc::new(Barrier::new(PUBLISHERS));

        let workers: Vec<_> = (0..PUBLISHERS)
            .map(|index| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                let payload = payload.clone();
                thread::spawn(move || {
                    let capability = publisher_token(index);
                    barrier.wait();
                    for replay in 0..REPLAYS {
                        // Every third attempt is abandoned first, so a retry after an
                        // abort races the retries that never aborted.
                        if replay % 3 == 0 {
                            let staged = store
                                .stage(CLASS, payload.clone(), &capability)
                                .expect("staging is authorized");
                            drop(staged);
                        }
                        store
                            .publish(CLASS, payload.clone(), &capability)
                            .expect("a replay is a fresh publication");
                    }
                })
            })
            .collect();

        for worker in workers {
            worker.join().expect("no publisher panicked");
        }

        let operator = token("operator");
        let view = store.audit_view(&operator).expect("operator");
        let identity = fnv1a_handle(&payload);
        assert_eq!(view.published_count(), 1);
        assert_eq!(
            view.receipts(&identity).len(),
            PUBLISHERS * REPLAYS,
            "receipts collapsed across replays"
        );
        assert_eq!(
            store.read(&identity, &token("reader")).expect("published"),
            payload
        );
        assert_eq!(
            view.storage_attribution(),
            BTreeMap::from([(CLASS, payload.len() as u64)]),
            "a replay must not re-store the content"
        );
        assert_eq!(view.fsck(), Vec::new());

        let costs: BTreeSet<_> = view
            .receipts(&identity)
            .iter()
            .map(PublicationReceipt::cost)
            .collect();
        assert_eq!(
            costs.len(),
            1,
            "convergence became visible in reported cost"
        );
        summaries.push(summarize(&view));
    }

    assert_deterministic(&summaries);
}

// --- axis 5c: capability administration racing publication -------------------------------

/// Minting and revoking while publication is in flight.
///
/// Revocation is not instantaneous and this does not pretend otherwise: a publication
/// authorized before a concurrent revoke may complete. What must hold is that the store
/// never ends up inconsistent — every receipt issued names a readable artifact, refusals
/// are capability denials rather than aborts, and `fsck` is clean.
#[test]
fn capability_administration_racing_publication_leaves_a_consistent_store() {
    const PUBLISHERS: usize = 24;
    const ROUNDS: usize = 4;
    let payload = b"published while capabilities churn".to_vec();

    for _round in 0..ROUNDS {
        let store = store_with(Fnv1aIdentifier, NoFaultsHere, PUBLISHERS);
        let barrier = Arc::new(Barrier::new(PUBLISHERS + 1));

        let mut workers: Vec<thread::JoinHandle<Vec<Result<PublicationReceipt, PublishRefusal>>>> =
            Vec::new();
        for index in 0..PUBLISHERS {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            let payload = payload.clone();
            workers.push(thread::spawn(move || {
                let capability = publisher_token(index);
                barrier.wait();
                vec![store.publish(CLASS, payload, &capability)]
            }));
        }

        let administrator = {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let operator = token("operator");
                barrier.wait();
                for index in 0..PUBLISHERS {
                    if index % 2 == 1 {
                        store
                            .revoke(&operator, &publisher_token(index))
                            .expect("operator may revoke");
                    }
                }
            })
        };

        let results: Vec<Result<PublicationReceipt, PublishRefusal>> = workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("no publisher panicked"))
            .collect();
        administrator.join().expect("administrator did not panic");

        let operator = token("operator");
        let reader = token("reader");
        let view = store.audit_view(&operator).expect("operator");
        let successes = results.iter().filter(|result| result.is_ok()).count();

        for result in &results {
            match result {
                Ok(receipt) => assert_eq!(
                    store.read(receipt.handle(), &reader).expect("readable"),
                    payload,
                    "a receipt issued during capability churn names an unreadable artifact"
                ),
                Err(PublishRefusal::CapabilityDenied(_)) => {}
                Err(PublishRefusal::Aborted(aborted)) => {
                    panic!("a revoked capability caused a storage abort: {aborted}")
                }
            }
        }

        assert!(successes > 0, "every publication was refused");
        assert_eq!(view.published_count(), 1);
        assert_eq!(view.receipts(&fnv1a_handle(&payload)).len(), successes);
        assert_eq!(
            view.aborts(),
            Vec::new(),
            "a capability denial must never be recorded as an abort"
        );
        assert_eq!(view.fsck(), Vec::new());
    }
}

/// Every publish attempt under contention is authorized exactly once and audited exactly
/// once, whatever the schedule (plan §18.5: no unaudited privileged operation).
#[test]
fn every_concurrent_publication_is_audited_exactly_once() {
    const PUBLISHERS: usize = 32;
    let payload = b"audited under contention".to_vec();

    let audit = Arc::new(AuditLog::new());
    let mut builder = ReferenceStore::builder(Fnv1aIdentifier, Arc::clone(&audit))
        .capability(grant("operator", "operator", AuthorityLevel::Promote));
    for index in 0..PUBLISHERS {
        builder = builder.capability(grant(
            &format!("k{index}"),
            &format!("publisher{index}"),
            AuthorityLevel::Propose,
        ));
    }
    let store = Arc::new(builder.build());
    let barrier = Arc::new(Barrier::new(PUBLISHERS));

    let workers: Vec<_> = (0..PUBLISHERS)
        .map(|index| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            let payload = payload.clone();
            thread::spawn(move || {
                let capability = publisher_token(index);
                barrier.wait();
                store.publish(CLASS, payload, &capability)
            })
        })
        .collect();

    for worker in workers {
        worker
            .join()
            .expect("no publisher panicked")
            .expect("published");
    }

    let publish_records: Vec<_> = audit
        .records()
        .into_iter()
        .filter(|record| record.action() == Action::Publish)
        .collect();
    assert_eq!(publish_records.len(), PUBLISHERS);
    let audited_actors: BTreeSet<ActorId> = publish_records
        .iter()
        .filter_map(|record| record.actor().cloned())
        .collect();
    assert_eq!(
        audited_actors.len(),
        PUBLISHERS,
        "an authorization record was lost or duplicated under contention"
    );
    assert!(
        publish_records
            .iter()
            .all(|record| record.handle().is_none()),
        "publication was audited with a handle, i.e. after the identity existed"
    );
}

// --- controls ----------------------------------------------------------------------------

/// The baseline the ledger races are measured against: garbage collection is correct on a
/// quiescent store.
///
/// Without this control the failures below would be evidence that GC is simply wrong,
/// which it is not. It reclaims exactly the crash residue and nothing else, every time,
/// as long as no publication is in flight.
#[test]
fn baseline_garbage_collection_of_a_quiescent_store_reclaims_only_crash_residue() {
    let store = store_with(Fnv1aIdentifier, NoFaultsHere, 1);
    let operator = token("operator");
    let reader = token("reader");
    let capability = publisher_token(0);

    let published = store
        .publish(CLASS, b"kept".to_vec(), &capability)
        .expect("published");

    // A crash between the commits, completed and settled before GC runs.
    let residue = {
        let committed = store
            .stage(CLASS, b"residue".to_vec(), &capability)
            .expect("staged")
            .commit_content()
            .expect("content commits");
        let handle = committed.handle().clone();
        drop(committed);
        handle
    };

    let reclaimed = store.collect_garbage(&operator).expect("operator");
    assert_eq!(reclaimed, vec![residue]);
    assert_eq!(
        store
            .read(published.handle(), &reader)
            .expect("still there"),
        b"kept"
    );
    let view = store.audit_view(&operator).expect("operator");
    assert_eq!(view.fsck(), Vec::new());
    assert_eq!(view.published_count(), 1);
    assert_eq!(
        view.storage_attribution(),
        BTreeMap::from([(CLASS, 4)]),
        "only the reachable artifact survives"
    );
}

// --- violations --------------------------------------------------------------------------
//
// The three tests below FAILED when this campaign was written (bn-21dd), and were retained
// `#[ignore]`d so that `just check` stayed honest while the defect was triaged — not
// because they were slow, flaky, or speculative. bn-2siid fixed the defect in
// `src/publication.rs`: the garbage collector's root set now pins publications that have
// committed content and not yet their index entry, so it can no longer reclaim content a
// publisher has already been told is durable.
//
// The `#[ignore]`s are therefore gone and these run on every `cargo test`. Do not delete
// them and do not weaken their assertions: they assert the G0-DX-13 pass condition
// verbatim, and each one still fails against the pre-fix store, which is the only thing
// that makes them worth keeping.

/// **Was a G0-DX-13 violation; fixed by bn-2siid.** A receipt must name a readable
/// artifact, even when garbage collection runs while the publication is between its two
/// commits.
///
/// As found, `ReferenceStore::collect_garbage` computed its root set from the index alone:
///
/// > let reachable: BTreeSet<ArtifactHandle> = state.index.values().cloned().collect();
///
/// A publication that has completed `commit_content` and not yet `commit_index` was, by
/// that definition, unreachable — so GC reclaimed content that a live publisher had
/// already been told was durable. The publisher then committed its index entry and
/// received a receipt for an artifact whose content was gone.
///
/// Three documented claims were false as a result:
///
/// - plan §4.5 / INV-017 — the artifact was visible (`published_count == 1`) with no
///   durable content behind it, which is the "dangling index entry" docs/35 calls "a lie
///   about what the store holds";
/// - `StoreDefect::MissingReferent`, documented as *"Never produced by this store's own
///   ordering — content is committed first — so finding one means external damage"*, was
///   produced by this store's own GC with no external damage at all;
/// - `ReferenceStore::collect_garbage`, documented as *"Collecting it cannot make a
///   published artifact unavailable"*, made one unavailable.
///
/// docs/35 gives the root set as "named roots, **live tasks**, receipts, and retention
/// policy", and the store's was neither: it omitted in-flight publications. The fix
/// (bn-2siid) pins an identity as a root for exactly as long as its `CommittedContent`
/// is alive; receipts turned out to need no root of their own, because a receipt is
/// appended in the same critical section that inserts the index entry naming it. This
/// test now runs in the default suite and is the regression guard for that.
#[test]
fn a_receipt_must_name_a_readable_artifact_when_gc_runs_concurrently() {
    let payload = b"published while the collector ran".to_vec();
    let (faults, reached, resume) = PauseFirstAt::new(PublicationPhase::CommittingIndex);
    let store = store_with(Fnv1aIdentifier, faults, 1);

    let publisher = {
        let store = Arc::clone(&store);
        let payload = payload.clone();
        thread::spawn(move || {
            let capability = publisher_token(0);
            store.publish(CLASS, payload, &capability)
        })
    };

    // The publisher's content is durable and its index entry is not yet written.
    reached
        .recv()
        .expect("the publisher reached the index commit");
    store
        .collect_garbage(&token("operator"))
        .expect("operator may collect");
    resume.send(()).expect("release the publisher");

    let receipt = publisher
        .join()
        .expect("no publisher panicked")
        .expect("the publication reported success");

    let operator = token("operator");
    let view = store.audit_view(&operator).expect("operator");

    assert_eq!(
        store.read(receipt.handle(), &token("reader")),
        Ok(payload),
        "a receipt was issued for an artifact the store cannot read back"
    );
    assert_eq!(
        view.fsck(),
        Vec::new(),
        "the store's own garbage collector produced a defect its ordering calls impossible"
    );
}

/// **Was a G0-DX-13 violation, sharper witness; fixed by bn-2siid.** The same race, with
/// an adversarial identifier, substituted one publisher's bytes for another's.
///
/// Sequence, forced through the fault seam and asserted only on its outcome:
///
/// 1. publisher A commits the content `original` and parks before its index commit;
/// 2. garbage collection reclaims `original` — A is in flight, so it is not a root;
/// 3. publisher B publishes `impostor`, which collides onto A's identity. ADR-0013's
///    exact comparison finds no existing content to compare against, because GC deleted
///    it, so the collision goes undetected and `impostor` is stored and indexed;
/// 4. A resumes, converges onto the existing index entry, and is handed a receipt.
///
/// A then held a receipt for an identity whose content was bytes A never published. That
/// is strictly worse than the missing referent above: the store is internally consistent,
/// `fsck` is clean, and the artifact reads — it just reads as somebody else's. Exact
/// comparison was not defeated by a better collision; it was bypassed by deleting the
/// value it was going to compare against.
///
/// # What step 3 must be instead, and why this test changed with the fix
///
/// Step 2 is the defect, and removing it necessarily changes step 3: with `original`
/// pinned by A's in-flight publication, it is still there for exact comparison to reject
/// `impostor` against, so B is *refused* with
/// [`AbortReason::IdentityCollision`] rather than taking A's identity. There is no correct
/// store in which B still succeeds — a success at A's identity with different bytes either
/// conflates two artifacts (ADR-0013, and the mutation campaign's `OverwriteOnCollision`)
/// or leaves B holding a receipt for A's bytes, which is the same lie this test is named
/// after. So the line that asserted B's success is now an assertion of B's refusal, which
/// is a *stronger* claim: the pre-fix store fails this test both ways, because there B
/// succeeded and A read back `impostor`. Everything else, including the final assertion,
/// is verbatim what the falsification campaign wrote (bn-21dd); the change is recorded on
/// bone bn-2siid.
#[test]
fn concurrent_gc_must_not_let_a_receipt_name_another_publishers_bytes() {
    let (faults, reached, resume) = PauseFirstAt::new(PublicationPhase::CommittingIndex);
    let store = store_with(TotalColliderIdentifier, faults, 2);

    let first = {
        let store = Arc::clone(&store);
        thread::spawn(move || {
            let capability = publisher_token(0);
            store.publish(CLASS, b"original".to_vec(), &capability)
        })
    };

    reached.recv().expect("the first publisher parked");
    store
        .collect_garbage(&token("operator"))
        .expect("operator may collect");
    let second = store.publish(CLASS, b"impostor".to_vec(), &publisher_token(1));
    resume.send(()).expect("release the first publisher");

    let first = first
        .join()
        .expect("no publisher panicked")
        .expect("the first publication reported success");

    assert_eq!(
        first.handle(),
        &ArtifactHandle::new(CLASS, "collision").expect("the collider's identity is legal"),
        "the identifier collides"
    );
    match second {
        Ok(receipt) => panic!(
            "the second publication found nothing to collide with: garbage collection \
             deleted the operand of ADR-0013's exact comparison, so {} was published from \
             bytes the publication that owns it never wrote",
            receipt.handle()
        ),
        Err(PublishRefusal::Aborted(aborted)) => {
            assert_eq!(
                aborted.reason(),
                AbortReason::IdentityCollision,
                "the colliding publication was refused for the wrong reason"
            );
            assert_eq!(aborted.phase(), PublicationPhase::CommittingContent);
        }
        Err(PublishRefusal::CapabilityDenied(_)) => {
            panic!("a collision was reported as a capability denial")
        }
    }
    assert_eq!(
        store.read(first.handle(), &token("reader")),
        Ok(b"original".to_vec()),
        "the first publisher's receipt names the second publisher's bytes"
    );
}

/// **Was a G0-DX-13 violation, unassisted; fixed by bn-2siid.** The same defect with no
/// rendezvous: plain threads publishing while a collector runs.
///
/// The two tests above force the interleaving through the fault seam, which invites the
/// objection that the seam manufactured the bug. It did not. This test uses no seam at
/// all — ordinary `publish` calls racing an ordinary `collect_garbage` loop — and against
/// the unfixed store the index verifier reported missing referents, and receipts named
/// artifacts that could not be read.
///
/// It is retained as corroboration only. The deterministic reproductions above are the
/// ones the fix was written against; this one is a probability argument, and a scheduler
/// that happened to serialize every publication would let it pass vacuously.
#[test]
fn unassisted_concurrent_gc_and_publication_lose_committed_content() {
    const PUBLISHERS: usize = 16;
    const ROUNDS: usize = 40;

    let mut complaints: Vec<String> = Vec::new();
    for round in 0..ROUNDS {
        let store = store_with(Fnv1aIdentifier, NoFaultsHere, PUBLISHERS);
        let barrier = Arc::new(Barrier::new(PUBLISHERS + 1));

        let publishers: Vec<_> = (0..PUBLISHERS)
            .map(|index| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let capability = publisher_token(index);
                    let payload = format!("round {round} payload {index}").into_bytes();
                    barrier.wait();
                    (payload.clone(), store.publish(CLASS, payload, &capability))
                })
            })
            .collect();

        let collector = {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let operator = token("operator");
                barrier.wait();
                for _ in 0..64 {
                    store
                        .collect_garbage(&operator)
                        .expect("operator may collect");
                }
            })
        };

        let outcomes: Vec<(Vec<u8>, Result<PublicationReceipt, PublishRefusal>)> = publishers
            .into_iter()
            .map(|worker| worker.join().expect("no publisher panicked"))
            .collect();
        collector.join().expect("collector did not panic");

        let reader = token("reader");
        for (payload, result) in &outcomes {
            if let Ok(receipt) = result
                && store.read(receipt.handle(), &reader) != Ok(payload.clone())
            {
                complaints.push(format!(
                    "round {round}: receipt {} names an unreadable artifact",
                    receipt.handle()
                ));
            }
        }
        for defect in store
            .audit_view(&token("operator"))
            .expect("operator")
            .fsck()
        {
            if let StoreDefect::MissingReferent(path) = defect {
                complaints.push(format!(
                    "round {round}: fsck reports missing referent {path}"
                ));
            }
        }
    }

    assert!(
        complaints.is_empty(),
        "publication concurrent with garbage collection lost committed content:\n{complaints:#?}"
    );
}

// --- shared helpers ----------------------------------------------------------------------

/// Storage that never fails, spelled locally so every fixture in this file reads the same.
#[derive(Debug, Clone, Copy)]
struct NoFaultsHere;

impl StorageFaults for NoFaultsHere {
    fn check(&self, _phase: PublicationPhase) -> Result<(), AbortReason> {
        Ok(())
    }
}

/// Storage that always fails at one phase.
#[derive(Debug, Clone, Copy)]
struct AlwaysFailAt(PublicationPhase);

impl StorageFaults for AlwaysFailAt {
    fn check(&self, phase: PublicationPhase) -> Result<(), AbortReason> {
        if phase == self.0 {
            Err(AbortReason::StorageExhausted)
        } else {
            Ok(())
        }
    }
}

/// The pass condition's third clause: the same inputs produce the same store, whatever
/// the schedule was.
fn assert_deterministic(summaries: &[String]) {
    assert!(
        summaries.windows(2).all(|pair| pair[0] == pair[1]),
        "the store's contents varied across identically-configured rounds:\n{summaries:#?}"
    );
}
