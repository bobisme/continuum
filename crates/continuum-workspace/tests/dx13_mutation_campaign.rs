//! G0-DX-13 mutation campaign: are the falsification checks load-bearing?
//!
//! # Why this file exists
//!
//! `notes/plan/notes/G0_SPIKE_MATRIX.md`'s promotion rule requires **adversarial
//! mutations** before a spike becomes architecture. A concurrency test that passes
//! proves nothing on its own — a test that asserts nothing also passes, and so does a
//! test whose assertions happen to hold for a broken store. The obligation is to show
//! the checks *fail* when the semantics are broken in ways a real implementer would
//! plausibly break them.
//!
//! `crates/continuum-workspace/src/` is not modified, here or anywhere in this campaign.
//! Instead this file contains [`MutantStore`] — a small, deliberately separate
//! re-implementation of the publication core — and runs the *same three checks* against
//! the real [`ReferenceStore`] and against each mutant. The checks are written once, as
//! functions over a [`PublicationUnderTest`] trait, so there is no possibility of the
//! reference being graded on an easier rubric than the mutants.
//!
//! # The three checks
//!
//! Each is one clause of the G0-DX-13 pass condition or of the ordering INV-017 rests on:
//!
//! | check | asserts |
//! |---|---|
//! | [`check_convergence_and_receipts`] | one semantic identity per distinct payload; a receipt per publisher; every receipt reads back the publisher's own bytes |
//! | [`check_atomic_abort`] | a refused content commit publishes nothing — no index entry, no readable artifact, no defect (docs/35's content-before-index ordering) |
//! | [`check_exact_comparison`] | colliding byte-different values are never conflated (ADR-0013) |
//!
//! # The four stores under test
//!
//! | store | convergence/receipts | atomic abort | exact comparison |
//! |---|---|---|---|
//! | [`ReferenceStore`] | pass | pass | pass |
//! | `Mutation::Faithful` (control) | pass | pass | pass |
//! | `Mutation::IndexBeforeContent` | pass | **caught** | pass |
//! | `Mutation::LastWriterWinsReceipts` | **caught** | pass | **caught** |
//! | `Mutation::OverwriteOnCollision` | pass | pass | **caught** |
//!
//! The passes matter as much as the failures: they show each check is aimed at one
//! property rather than rejecting anything unfamiliar, and the `Faithful` control shows
//! the checks do not simply reject hand-written stores. The one off-diagonal catch —
//! `LastWriterWinsReceipts` also failing the collision check — is reported rather than
//! papered over: that check counts receipts against winners, so a clobbering ledger
//! trips it too, and the test asserts *which* complaint each check produced so a
//! coincidental catch cannot be mistaken for a targeted one.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Barrier, Mutex, MutexGuard, PoisonError};
use std::thread;

use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle, ArtifactPath};
use continuum_workspace::publication::{
    AbortReason, ActorId, AuditLog, AuthorityLevel, CapabilityDescriptor, CapabilityToken,
    ContentIdentifier, IdentityUnavailable, PublicationPhase, ReferenceStore, StorageFaults,
    StoreDefect,
};

/// The class every payload here is published under.
const CLASS: ArtifactClass = ArtifactClass::Evidence;

// --- identity functions ------------------------------------------------------------------

/// A total, deterministic identity: FNV-1a over the content.
fn fnv1a_identity(class: ArtifactClass, content: &[u8]) -> ArtifactHandle {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in content {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    ArtifactHandle::new(class, &format!("{hash:016x}")).expect("legal identity")
}

/// An adversarial identity: everything collides.
fn colliding_identity(class: ArtifactClass, _content: &[u8]) -> ArtifactHandle {
    ArtifactHandle::new(class, "collision").expect("legal identity")
}

/// [`fnv1a_identity`] as a [`ContentIdentifier`], for the real store.
#[derive(Debug, Clone, Copy)]
struct Fnv1aIdentifier;

impl ContentIdentifier for Fnv1aIdentifier {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        Ok(fnv1a_identity(class, content))
    }
}

/// [`colliding_identity`] as a [`ContentIdentifier`], for the real store.
#[derive(Debug, Clone, Copy)]
struct CollidingIdentifier;

impl ContentIdentifier for CollidingIdentifier {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        Ok(colliding_identity(class, content))
    }
}

/// Storage that refuses the content commit, and nothing else.
#[derive(Debug, Clone, Copy)]
struct FailContentCommit;

impl StorageFaults for FailContentCommit {
    fn check(&self, phase: PublicationPhase) -> Result<(), AbortReason> {
        if phase == PublicationPhase::CommittingContent {
            Err(AbortReason::StorageExhausted)
        } else {
            Ok(())
        }
    }
}

/// Storage that never fails.
#[derive(Debug, Clone, Copy)]
struct NeverFails;

impl StorageFaults for NeverFails {
    fn check(&self, _phase: PublicationPhase) -> Result<(), AbortReason> {
        Ok(())
    }
}

// --- the seam the checks are written against ---------------------------------------------

/// The publication surface the three checks need, and nothing more.
///
/// Deliberately minimal: the checks must be expressible against a store that is *not*
/// [`ReferenceStore`], or the mutation campaign would be grading the reference against
/// itself.
trait PublicationUnderTest: Sync {
    /// Publish `content` as `publisher`, returning the identity it was filed under.
    fn publish(&self, publisher: usize, content: Vec<u8>) -> Result<ArtifactHandle, Refused>;

    /// Read a published artifact, or `None` if nothing is published at that identity.
    fn read(&self, handle: &ArtifactHandle) -> Option<Vec<u8>>;

    /// The publishers named by the receipts filed under `handle`, in ledger order.
    fn receipt_publishers(&self, handle: &ArtifactHandle) -> Vec<String>;

    /// Every published identity.
    fn identities(&self) -> Vec<ArtifactHandle>;

    /// The index verifier's findings, rendered as strings.
    fn defects(&self) -> Vec<String>;
}

/// A publication that produced no receipt. Why is not this trait's business.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Refused;

/// The name a publisher index publishes under.
fn publisher_name(publisher: usize) -> String {
    format!("publisher{publisher}")
}

// --- the checks --------------------------------------------------------------------------

/// **One semantic identity; no lost receipts.**
///
/// `publishers_per_payload` threads publish each payload simultaneously. Afterwards the
/// store must hold exactly one identity per distinct payload, one receipt per publisher
/// under each, and each identity must read back the exact bytes it was published from.
fn check_convergence_and_receipts(
    store: &(impl PublicationUnderTest + ?Sized),
    payloads: &[Vec<u8>],
    publishers_per_payload: usize,
    identify: fn(ArtifactClass, &[u8]) -> ArtifactHandle,
) -> Result<(), String> {
    let total = payloads.len() * publishers_per_payload;
    let barrier = Barrier::new(total);

    let outcomes: Vec<(usize, Vec<u8>, Result<ArtifactHandle, Refused>)> = thread::scope(|scope| {
        let workers: Vec<_> = (0..total)
            .map(|publisher| {
                let payload = payloads[publisher % payloads.len()].clone();
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    let result = store.publish(publisher, payload.clone());
                    (publisher, payload, result)
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().expect("no publisher panicked"))
            .collect()
    });

    for (publisher, _, result) in &outcomes {
        if result.is_err() {
            return Err(format!(
                "publisher {publisher} was refused with no fault injected"
            ));
        }
    }

    let identities: BTreeSet<ArtifactHandle> = store.identities().into_iter().collect();
    if identities.len() != payloads.len() {
        return Err(format!(
            "one semantic identity violated: {} distinct payloads produced {} identities",
            payloads.len(),
            identities.len()
        ));
    }

    for payload in payloads {
        let handle = identify(CLASS, payload);
        if !identities.contains(&handle) {
            return Err(format!("{handle} is not published, but its payload was"));
        }
        match store.read(&handle) {
            Some(bytes) if &bytes == payload => {}
            other => {
                return Err(format!(
                    "{handle} reads back {other:?}, not the bytes it was published from"
                ));
            }
        }

        let receipts = store.receipt_publishers(&handle);
        if receipts.len() != publishers_per_payload {
            return Err(format!(
                "receipts lost for {handle}: expected {publishers_per_payload}, ledger holds {}",
                receipts.len()
            ));
        }
        let distinct: BTreeSet<&String> = receipts.iter().collect();
        if distinct.len() != publishers_per_payload {
            return Err(format!(
                "receipts for {handle} do not name {publishers_per_payload} distinct publishers"
            ));
        }
    }

    let defects = store.defects();
    if !defects.is_empty() {
        return Err(format!("index verifier found defects: {defects:?}"));
    }
    Ok(())
}

/// **A refused content commit publishes nothing.**
///
/// docs/35: content is committed before the index entry that names it, so a failure at
/// the content commit must leave no index entry, nothing readable, and no defect. This is
/// the check that a store writing its index first cannot pass.
fn check_atomic_abort(
    store: &(impl PublicationUnderTest + ?Sized),
    payload: &[u8],
    identify: fn(ArtifactClass, &[u8]) -> ArtifactHandle,
) -> Result<(), String> {
    let handle = identify(CLASS, payload);
    if store.publish(0, payload.to_vec()).is_ok() {
        return Err("a refused content commit still issued a receipt".to_owned());
    }
    if !store.identities().is_empty() {
        return Err(format!(
            "a refused publication left index entries: {:?}",
            store.identities()
        ));
    }
    if let Some(bytes) = store.read(&handle) {
        return Err(format!(
            "a refused publication left {} readable bytes at {handle}",
            bytes.len()
        ));
    }
    if !store.receipt_publishers(&handle).is_empty() {
        return Err(format!("a refused publication left a receipt at {handle}"));
    }
    let defects = store.defects();
    if !defects.is_empty() {
        return Err(format!(
            "a refused publication left the store defective: {defects:?}"
        ));
    }
    Ok(())
}

/// **Colliding byte-different values are never conflated (ADR-0013).**
///
/// Under an identifier that maps everything to one identity, publishers of different
/// bytes contend for the same slot. Whoever wins is the schedule's business; what is
/// checked is that exactly one payload is stored, that every publisher holding a receipt
/// published exactly those bytes, and that the losers hold none.
fn check_exact_comparison(
    store: &(impl PublicationUnderTest + ?Sized),
    payloads: &[Vec<u8>],
    publishers_per_payload: usize,
) -> Result<(), String> {
    let total = payloads.len() * publishers_per_payload;
    let barrier = Barrier::new(total);

    let outcomes: Vec<(Vec<u8>, Result<ArtifactHandle, Refused>)> = thread::scope(|scope| {
        let workers: Vec<_> = (0..total)
            .map(|publisher| {
                let payload = payloads[publisher % payloads.len()].clone();
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    let result = store.publish(publisher, payload.clone());
                    (payload, result)
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().expect("no publisher panicked"))
            .collect()
    });

    let identities = store.identities();
    if identities.len() != 1 {
        return Err(format!(
            "a total collision produced {} identities, not one",
            identities.len()
        ));
    }
    let identity = identities[0].clone();
    let stored = store
        .read(&identity)
        .ok_or_else(|| format!("{identity} is indexed but unreadable"))?;

    let mut winners = 0usize;
    for (payload, result) in &outcomes {
        if result.is_ok() {
            winners += 1;
            if payload != &stored {
                return Err(format!(
                    "a publisher of {} bytes holds a receipt for an identity storing {} other bytes: \
                     two artifacts were conflated (ADR-0013)",
                    payload.len(),
                    stored.len()
                ));
            }
        }
    }
    if winners == 0 {
        return Err("nobody won the collision, so nothing was published".to_owned());
    }
    if store.receipt_publishers(&identity).len() != winners {
        return Err(format!(
            "receipts lost under collision: {winners} winners, {} receipts",
            store.receipt_publishers(&identity).len()
        ));
    }
    Ok(())
}

// --- the real store, behind the seam ------------------------------------------------------

/// [`ReferenceStore`] as a [`PublicationUnderTest`].
struct ReferenceUnderTest {
    store: ReferenceStore,
    publishers: usize,
    /// The declared identifier `fsck` is checked against — a copy of the same seam the
    /// store was built with, since this harness tests the store's own mechanics rather
    /// than a substituted-seam scenario.
    declared: Box<dyn ContentIdentifier>,
}

impl ReferenceUnderTest {
    fn new(
        identifier: impl ContentIdentifier + Clone + 'static,
        faults: impl StorageFaults + 'static,
        publishers: usize,
    ) -> Self {
        let declared: Box<dyn ContentIdentifier> = Box::new(identifier.clone());
        let mut builder = ReferenceStore::builder(identifier, Arc::new(AuditLog::new()))
            .faults(faults)
            .capability(CapabilityDescriptor::new(
                mint("operator"),
                ActorId::new("operator"),
                AuthorityLevel::Promote,
            ));
        for publisher in 0..publishers {
            builder = builder.capability(CapabilityDescriptor::new(
                mint(&format!("k{publisher}")),
                ActorId::new(&publisher_name(publisher)),
                AuthorityLevel::Propose,
            ));
        }
        Self {
            store: builder.build(),
            publishers,
            declared,
        }
    }
}

/// Mint a capability token from a test-chosen identity (entropy is a capability, INV-005).
fn mint(identity: &str) -> CapabilityToken {
    CapabilityToken::mint(identity).expect("test capability identity is well formed")
}

impl PublicationUnderTest for ReferenceUnderTest {
    fn publish(&self, publisher: usize, content: Vec<u8>) -> Result<ArtifactHandle, Refused> {
        assert!(publisher < self.publishers, "publisher has no capability");
        self.store
            .publish(CLASS, content, &mint(&format!("k{publisher}")))
            .map(|receipt| receipt.handle().clone())
            .map_err(|_| Refused)
    }

    fn read(&self, handle: &ArtifactHandle) -> Option<Vec<u8>> {
        // The operator holds `promote`, which is at least `read`, so a refusal here means
        // "not published" and never "not authorized".
        self.store.read(handle, &mint("operator")).ok()
    }

    fn receipt_publishers(&self, handle: &ArtifactHandle) -> Vec<String> {
        self.store
            .audit_view(&mint("operator"))
            .expect("operator may audit")
            .receipts(handle)
            .iter()
            .map(|receipt| receipt.actor().to_string())
            .collect()
    }

    fn identities(&self) -> Vec<ArtifactHandle> {
        self.store
            .audit_view(&mint("operator"))
            .expect("operator may audit")
            .identities()
    }

    fn defects(&self) -> Vec<String> {
        self.store
            .audit_view(&mint("operator"))
            .expect("operator may audit")
            .fsck(self.declared.as_ref())
            .iter()
            .map(StoreDefect::to_string)
            .collect()
    }
}

// --- the mutants ---------------------------------------------------------------------------

/// One plausible way an implementer breaks the publication core.
///
/// Each is a bug someone would actually write, not a strawman: inverting the two commits
/// is the natural order if you think of the index as the artifact; overwriting the ledger
/// entry is what `insert` does when you reach for a map instead of a log; overwriting
/// colliding content is what "content addressing means equal hashes are equal content"
/// gets you.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mutation {
    /// No bug. The control: a faithful re-implementation must pass every check, or the
    /// checks are rejecting unfamiliarity rather than incorrectness.
    Faithful,
    /// The index entry is written before the content commit, inverting docs/35's ordering.
    IndexBeforeContent,
    /// The receipt ledger keeps only the latest receipt per identity instead of appending.
    LastWriterWinsReceipts,
    /// Byte-different content arriving at an existing identity overwrites it instead of
    /// aborting with `IdentityCollision`.
    OverwriteOnCollision,
}

#[derive(Debug, Default)]
struct MutantState {
    content: BTreeMap<ArtifactHandle, Vec<u8>>,
    index: BTreeMap<ArtifactPath, ArtifactHandle>,
    ledger: BTreeMap<ArtifactHandle, Vec<String>>,
}

/// A separate, deliberately buggy implementation of the publication core.
///
/// It exists only to prove the checks above are load-bearing. It is a *copy* rather than
/// an edit: `crates/continuum-workspace/src/publication.rs` is untouched by this campaign,
/// so there is nothing to revert and no window in which the shipped code was wrong.
struct MutantStore {
    identify: fn(ArtifactClass, &[u8]) -> ArtifactHandle,
    mutation: Mutation,
    refuse_content_commit: bool,
    state: Mutex<MutantState>,
}

impl MutantStore {
    fn new(
        identify: fn(ArtifactClass, &[u8]) -> ArtifactHandle,
        mutation: Mutation,
        refuse_content_commit: bool,
    ) -> Self {
        Self {
            identify,
            mutation,
            refuse_content_commit,
            state: Mutex::new(MutantState::default()),
        }
    }

    fn state(&self) -> MutexGuard<'_, MutantState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl PublicationUnderTest for MutantStore {
    fn publish(&self, publisher: usize, content: Vec<u8>) -> Result<ArtifactHandle, Refused> {
        let handle = (self.identify)(CLASS, &content);
        let path = ArtifactPath::for_handle(&handle).map_err(|_| Refused)?;
        let mut state = self.state();

        if self.mutation == Mutation::IndexBeforeContent {
            state
                .index
                .entry(path.clone())
                .or_insert_with(|| handle.clone());
        }

        if self.refuse_content_commit {
            return Err(Refused);
        }

        match state.content.get(&handle) {
            Some(existing) if existing != &content => {
                if self.mutation == Mutation::OverwriteOnCollision {
                    state.content.insert(handle.clone(), content);
                } else {
                    return Err(Refused);
                }
            }
            Some(_) => {}
            None => {
                state.content.insert(handle.clone(), content);
            }
        }

        if self.mutation != Mutation::IndexBeforeContent {
            state.index.entry(path).or_insert_with(|| handle.clone());
        }

        let name = publisher_name(publisher);
        if self.mutation == Mutation::LastWriterWinsReceipts {
            state.ledger.insert(handle.clone(), vec![name]);
        } else {
            state.ledger.entry(handle.clone()).or_default().push(name);
        }
        Ok(handle)
    }

    fn read(&self, handle: &ArtifactHandle) -> Option<Vec<u8>> {
        let path = ArtifactPath::for_handle(handle).ok()?;
        let state = self.state();
        match state.index.get(&path) {
            Some(indexed) if indexed == handle => state.content.get(handle).cloned(),
            _ => None,
        }
    }

    fn receipt_publishers(&self, handle: &ArtifactHandle) -> Vec<String> {
        self.state().ledger.get(handle).cloned().unwrap_or_default()
    }

    fn identities(&self) -> Vec<ArtifactHandle> {
        self.state().index.values().cloned().collect()
    }

    fn defects(&self) -> Vec<String> {
        let state = self.state();
        let reachable: BTreeSet<&ArtifactHandle> = state.index.values().collect();
        let mut defects = Vec::new();
        for (handle, bytes) in &state.content {
            if !reachable.contains(handle) {
                defects.push(format!("unreachable content: {handle}"));
            }
            if &(self.identify)(handle.class(), bytes) != handle {
                defects.push(format!("identity mismatch: {handle}"));
            }
        }
        for (path, handle) in &state.index {
            if !state.content.contains_key(handle) {
                defects.push(format!("missing referent: {path}"));
            }
        }
        defects.sort();
        defects
    }
}

// --- the campaign ----------------------------------------------------------------------

/// Payloads for the convergence check: distinct bytes, so FNV keeps them apart.
fn distinct_payloads() -> Vec<Vec<u8>> {
    (0..4u8)
        .map(|n| format!("mutation payload {n}").into_bytes())
        .collect()
}

/// Payloads for the collision check: byte-different, forced onto one identity.
fn colliding_payloads() -> Vec<Vec<u8>> {
    vec![b"original".to_vec(), b"impostor".to_vec()]
}

/// Every check against every store, as three `Result`s.
struct Verdict {
    convergence: Result<(), String>,
    atomic_abort: Result<(), String>,
    exact_comparison: Result<(), String>,
}

/// Grade one mutation (or the reference, when `mutation` is `None`).
fn grade(mutation: Option<Mutation>) -> Verdict {
    const PUBLISHERS_PER_PAYLOAD: usize = 6;
    let payloads = distinct_payloads();
    let total = payloads.len() * PUBLISHERS_PER_PAYLOAD;
    let colliding = colliding_payloads();
    let colliding_total = colliding.len() * PUBLISHERS_PER_PAYLOAD;

    match mutation {
        None => Verdict {
            convergence: check_convergence_and_receipts(
                &ReferenceUnderTest::new(Fnv1aIdentifier, NeverFails, total),
                &payloads,
                PUBLISHERS_PER_PAYLOAD,
                fnv1a_identity,
            ),
            atomic_abort: check_atomic_abort(
                &ReferenceUnderTest::new(Fnv1aIdentifier, FailContentCommit, 1),
                b"refused at the content commit",
                fnv1a_identity,
            ),
            exact_comparison: check_exact_comparison(
                &ReferenceUnderTest::new(CollidingIdentifier, NeverFails, colliding_total),
                &colliding,
                PUBLISHERS_PER_PAYLOAD,
            ),
        },
        Some(mutation) => Verdict {
            convergence: check_convergence_and_receipts(
                &MutantStore::new(fnv1a_identity, mutation, false),
                &payloads,
                PUBLISHERS_PER_PAYLOAD,
                fnv1a_identity,
            ),
            atomic_abort: check_atomic_abort(
                &MutantStore::new(fnv1a_identity, mutation, true),
                b"refused at the content commit",
                fnv1a_identity,
            ),
            exact_comparison: check_exact_comparison(
                &MutantStore::new(colliding_identity, mutation, false),
                &colliding,
                PUBLISHERS_PER_PAYLOAD,
            ),
        },
    }
}

#[test]
fn the_reference_implementation_passes_every_check() {
    let verdict = grade(None);
    verdict.convergence.expect("convergence and receipts");
    verdict.atomic_abort.expect("atomic abort");
    verdict.exact_comparison.expect("exact comparison");
}

#[test]
fn a_faithful_re_implementation_passes_every_check() {
    // The control. Without it, a mutant failing a check could mean nothing more than
    // "this is not the reference store".
    let verdict = grade(Some(Mutation::Faithful));
    verdict.convergence.expect("convergence and receipts");
    verdict.atomic_abort.expect("atomic abort");
    verdict.exact_comparison.expect("exact comparison");
}

#[test]
fn mutation_index_before_content_is_caught_by_the_atomic_abort_check() {
    let verdict = grade(Some(Mutation::IndexBeforeContent));
    let caught = verdict
        .atomic_abort
        .expect_err("writing the index first must be caught");
    assert!(
        caught.contains("index entries") || caught.contains("defective"),
        "caught for the wrong reason: {caught}"
    );

    // Targeted, not indiscriminate: the other two properties are untouched by this bug.
    verdict
        .convergence
        .expect("inverting the commits does not lose receipts");
    verdict
        .exact_comparison
        .expect("inverting the commits does not conflate artifacts");
}

#[test]
fn mutation_last_writer_wins_receipts_is_caught_by_the_no_lost_receipts_check() {
    let verdict = grade(Some(Mutation::LastWriterWinsReceipts));
    let caught = verdict
        .convergence
        .expect_err("a clobbering receipt ledger must be caught");
    assert!(
        caught.contains("receipts lost"),
        "caught for the wrong reason: {caught}"
    );

    verdict
        .atomic_abort
        .expect("a clobbering ledger does not break abort atomicity");
    // The collision check independently notices the lost receipts, which is honest to
    // report: it counts receipts against winners.
    let also_caught = verdict
        .exact_comparison
        .expect_err("the collision check counts receipts too");
    assert!(also_caught.contains("receipts lost"), "{also_caught}");
}

#[test]
fn mutation_overwrite_on_collision_is_caught_by_the_exact_comparison_check() {
    let verdict = grade(Some(Mutation::OverwriteOnCollision));
    let caught = verdict
        .exact_comparison
        .expect_err("conflating two artifacts must be caught");
    assert!(
        caught.contains("conflated"),
        "caught for the wrong reason: {caught}"
    );

    verdict
        .convergence
        .expect("overwriting on collision is invisible when nothing collides");
    verdict
        .atomic_abort
        .expect("overwriting on collision does not break abort atomicity");
}

#[test]
fn every_mutation_is_caught_by_at_least_one_check() {
    // The campaign's summary claim, stated once so a future mutation cannot be added
    // without also being caught.
    for mutation in [
        Mutation::IndexBeforeContent,
        Mutation::LastWriterWinsReceipts,
        Mutation::OverwriteOnCollision,
    ] {
        let verdict = grade(Some(mutation));
        assert!(
            verdict.convergence.is_err()
                || verdict.atomic_abort.is_err()
                || verdict.exact_comparison.is_err(),
            "{mutation:?} survived every check: the checks are not load-bearing"
        );
    }
}
