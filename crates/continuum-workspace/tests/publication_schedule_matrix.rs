//! **PHASE-A-DEL-03 (bn-cho5): the CAS two-phase machine under exhaustively enumerated
//! schedules.**
//!
//! The G0-DX-13 campaign (`tests/dx13_falsification.rs`) attacks a genuinely thread-safe
//! store by *forcing* interleavings — barriers and rendezvous — and asserting on outcomes.
//! `crates/continuum-task/src/region/schedule.rs` states the stronger discipline this
//! repository reached afterwards: where the state machine is reified as data, an
//! adversarial interleaving is a value a test can *enumerate*, and a property can be
//! asserted over **every** member of the schedule space rather than over the points a
//! rendezvous happens to reach.
//!
//! The publication machine qualifies: its two-phase commit is reified as values —
//! [`StagedPublication`] and [`CommittedContent`] are first-class, so a test can hold N
//! publications suspended at any phase and step them in any order, on one thread, with no
//! timing anywhere. This file enumerates the shuffle product of per-publisher phase
//! programs (plus garbage-collection and reader programs woven in) and asserts the
//! docs/35 ordering, the G0-DX-13 pass condition, and the INV-017 visibility rule at
//! every schedule.
//!
//! What this file **cannot** exercise — and does not claim to — is the memory-model
//! level: each phase step here runs a whole critical section atomically, exactly as the
//! store's own `Mutex` does. The residual (that the `Mutex` really does make those
//! sections atomic under real threads) is the sanitizer lane's business: `just
//! sanitizers` runs this crate's threaded suites under ASan and TSan.
//!
//! # House rules, inherited from the DX-13/DX-14 campaigns
//!
//! - **assertions are on outcomes of a written-down schedule, never on timing** — there
//!   are no threads in this file at all;
//! - **every sweep carries an anti-vacuity guard** — the size of each enumerated space is
//!   asserted as a literal (the multinomial coefficient), and a summary that came out
//!   empty fails;
//! - **bounded by construction** — the largest space here is 1,680 schedules; nothing is
//!   built by doubling;
//! - **`src/` is untouched** — the machine is driven through its public API only.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | three racing publishers of one payload converge at every one of 1,680 schedules | [`positive_identical_publishers_converge_at_every_interleaving`] |
//! | visibility is the index write, monotone, at every one of 560 reader weavings | [`positive_a_reader_sees_the_artifact_exactly_from_the_index_commit_on`] |
//! | collection never reclaims the in-flight window, wherever it lands | [`positive_collection_never_reclaims_a_pinned_publication_at_any_position`] |
//! | an abandoned second commit is residue: fsck names it, collection reclaims it | [`positive_an_abandoned_publication_is_typed_residue_wherever_collection_lands`] |
//! | a byte-different collision has one winner and a typed loser at every schedule | [`positive_a_collision_has_one_winner_and_a_typed_loser_at_every_interleaving`] |
//! | the whole matrix renders byte-identically across two runs | [`positive_the_whole_matrix_renders_byte_identically_across_two_runs`] |

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{
    AbortReason, ActorId, AuditLog, AuthorityLevel, CapabilityDescriptor, CapabilityToken,
    CommittedContent, ContentIdentifier, IdentityUnavailable, PublicationReceipt, ReferenceStore,
    StagedPublication, StoreDefect,
};

/// The class every payload here is published under.
const CLASS: ArtifactClass = ArtifactClass::Evidence;

// --- identity, forked from `dx13_mutation_campaign.rs` (a tests/ file is its own crate) ---

/// A total, deterministic identity: FNV-1a over the content.
fn fnv1a_identity(class: ArtifactClass, content: &[u8]) -> ArtifactHandle {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in content {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    ArtifactHandle::new(class, &format!("{hash:016x}")).expect("legal identity")
}

/// [`fnv1a_identity`] as a [`ContentIdentifier`].
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

/// An adversarial identity: everything collides.
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

/// Mint a token from a test-chosen identity (entropy is a capability, INV-005).
fn mint(identity: &str) -> CapabilityToken {
    CapabilityToken::mint(identity).expect("test capability identity is well formed")
}

/// A store with `publishers` propose-level publishers and one promote-level operator.
fn store_with(identifier: impl ContentIdentifier + 'static, publishers: usize) -> ReferenceStore {
    let mut builder = ReferenceStore::builder(identifier, Arc::new(AuditLog::new())).capability(
        CapabilityDescriptor::new(
            mint("operator"),
            ActorId::new("operator"),
            AuthorityLevel::Promote,
        ),
    );
    for publisher in 0..publishers {
        builder = builder.capability(CapabilityDescriptor::new(
            mint(&format!("k{publisher}")),
            ActorId::new(&format!("publisher{publisher}")),
            AuthorityLevel::Propose,
        ));
    }
    builder.build()
}

// --- schedules as data ---------------------------------------------------------------------

/// Every interleaving of programs with the given lengths, as sequences of program indices.
///
/// The shuffle product, enumerated the way `continuum_task::region::schedule::interleavings`
/// enumerates its own: deterministically, preserving each program's internal order, linear
/// in the output. (That function is typed over region [`Step`]s and a `tests/*.rs` file is
/// its own crate, so the enumerator is restated here over indices — the shape every program
/// in this file shares.)
fn weavings(lengths: &[usize]) -> Vec<Vec<usize>> {
    fn extend(remaining: &mut Vec<usize>, prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if remaining.iter().all(|left| *left == 0) {
            out.push(prefix.clone());
            return;
        }
        for program in 0..remaining.len() {
            if remaining[program] == 0 {
                continue;
            }
            remaining[program] -= 1;
            prefix.push(program);
            extend(remaining, prefix, out);
            prefix.pop();
            remaining[program] += 1;
        }
    }
    let mut out = Vec::new();
    extend(&mut lengths.to_vec(), &mut Vec::new(), &mut out);
    out
}

/// n! / (l0! · l1! · …), computed the boring way for the anti-vacuity literals.
fn multinomial(lengths: &[usize]) -> usize {
    let total: usize = lengths.iter().sum();
    let mut numerator: u128 = 1;
    for n in 1..=total {
        numerator *= n as u128;
    }
    for length in lengths {
        for n in 1..=*length {
            numerator /= n as u128;
        }
    }
    usize::try_from(numerator).expect("the spaces in this file are small by construction")
}

/// Where one publication stands between steps. The machine's own states, held as values.
enum Slot<'store> {
    Idle,
    Staged(StagedPublication<'store>),
    Committed(CommittedContent<'store>),
    Published(PublicationReceipt),
    /// The publication aborted (recorded reason) or was deliberately abandoned; later
    /// phases of its program are no-ops, mirroring `Schedule::run`'s continue-past-refusal.
    Settled(Option<AbortReason>),
}

impl Slot<'_> {
    fn receipt(&self) -> Option<&PublicationReceipt> {
        match self {
            Slot::Published(receipt) => Some(receipt),
            _ => None,
        }
    }

    fn abort_reason(&self) -> Option<AbortReason> {
        match self {
            Slot::Settled(reason) => *reason,
            _ => None,
        }
    }
}

/// One publisher's next move, applied to its slot. Returns the slot after the move.
///
/// A move against a slot that already settled is recorded as a no-op rather than a panic:
/// a schedule wants to keep going after a refusal to check the refusal corrupted nothing.
fn step<'store>(
    store: &'store ReferenceStore,
    slot: Slot<'store>,
    phase: Phase,
    publisher: usize,
    payload: &[u8],
) -> Slot<'store> {
    match (slot, phase) {
        (Slot::Idle, Phase::Stage) => {
            match store.stage(CLASS, payload.to_vec(), &mint(&format!("k{publisher}"))) {
                Ok(staged) => Slot::Staged(staged),
                Err(_) => Slot::Settled(None),
            }
        }
        (Slot::Staged(staged), Phase::CommitContent) => match staged.commit_content() {
            Ok(committed) => Slot::Committed(committed),
            Err(aborted) => Slot::Settled(Some(aborted.reason())),
        },
        (Slot::Committed(committed), Phase::CommitIndex) => match committed.commit_index() {
            Ok(receipt) => Slot::Published(receipt),
            Err(aborted) => Slot::Settled(Some(aborted.reason())),
        },
        (Slot::Committed(committed), Phase::Abandon) => {
            Slot::Settled(Some(committed.abandon().reason()))
        }
        (settled @ Slot::Settled(_), _) => settled,
        (_, phase) => panic!("a program reached {phase:?} out of order: the schedule is malformed"),
    }
}

/// One phase of the two-phase machine, as a program writes it down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Stage,
    CommitContent,
    CommitIndex,
    /// `CommittedContent::abandon` — the modeled crash between the two commits.
    Abandon,
}

/// The full three-step program.
const PUBLISH: [Phase; 3] = [Phase::Stage, Phase::CommitContent, Phase::CommitIndex];

/// The half-publication program: durable content, no index entry, ever.
const ABANDON: [Phase; 3] = [Phase::Stage, Phase::CommitContent, Phase::Abandon];

/// A schedule-independent description of a finished store, sorted where arrival order is
/// genuinely nondeterministic (the DX-13 rule: what must be deterministic is what the
/// store holds, not the order the schedule reached it).
fn summarize(store: &ReferenceStore) -> String {
    let view = store
        .audit_view(&mint("operator"))
        .expect("operator may audit");
    let mut receipts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for identity in view.identities() {
        let mut actors: Vec<String> = view
            .receipts(&identity)
            .iter()
            .map(|receipt| receipt.actor().to_string())
            .collect();
        actors.sort();
        receipts.insert(identity.to_string(), actors);
    }
    let mut aborts: Vec<String> = view
        .aborts()
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    aborts.sort();
    format!(
        "identities={:?} receipts={receipts:?} attribution={:?} fsck={:?} aborts={aborts:?}",
        view.identities(),
        view.storage_attribution(),
        view.fsck(&Fnv1aIdentifier),
    )
}

// --- sweep 1: publication races / duplicate coalescing --------------------------------------

/// **Three publishers of one payload, every schedule.** The G0-DX-13 pass condition —
/// one semantic identity, no lost receipts, deterministic result — asserted over the
/// whole shuffle product of the three phase programs, not over the points a rendezvous
/// reached: 9!/(3!·3!·3!) = 1,680 schedules.
///
/// This is duplicate coalescing stated exhaustively: whatever order the three
/// publications' phases land in, the store holds one artifact and every publisher's
/// receipt survives.
#[test]
fn positive_identical_publishers_converge_at_every_interleaving() {
    const PAYLOAD: &[u8] = b"identical bytes";
    let programs = [PUBLISH.to_vec(), PUBLISH.to_vec(), PUBLISH.to_vec()];
    let lengths = [3, 3, 3];
    let schedules = weavings(&lengths);
    assert_eq!(schedules.len(), multinomial(&lengths));
    assert_eq!(schedules.len(), 1_680, "9!/(3!·3!·3!)");

    let mut summaries: BTreeSet<String> = BTreeSet::new();
    for schedule in &schedules {
        let store = store_with(Fnv1aIdentifier, programs.len());
        let mut slots = [Slot::Idle, Slot::Idle, Slot::Idle];
        let mut cursors = [0_usize; 3];
        for &publisher in schedule {
            let phase = programs[publisher][cursors[publisher]];
            cursors[publisher] += 1;
            let slot = std::mem::replace(&mut slots[publisher], Slot::Idle);
            slots[publisher] = step(&store, slot, phase, publisher, PAYLOAD);
        }

        // Every publisher holds a receipt, and every receipt names one identity.
        let identities: BTreeSet<&ArtifactHandle> = slots
            .iter()
            .map(|slot| slot.receipt().expect("every publisher succeeds").handle())
            .collect();
        assert_eq!(identities.len(), 1, "publishers disagreed on identity");
        let identity = (*identities.iter().next().expect("one identity")).clone();

        let view = store.audit_view(&mint("operator")).expect("operator");
        assert_eq!(view.published_count(), 1);
        let ledger = view.receipts(&identity);
        assert_eq!(ledger.len(), 3, "a receipt was lost to the schedule");
        let actors: BTreeSet<String> = ledger
            .iter()
            .map(|receipt| receipt.actor().to_string())
            .collect();
        assert_eq!(actors.len(), 3, "two receipts name one publisher");
        assert_eq!(view.fsck(&Fnv1aIdentifier), Vec::new());
        assert_eq!(view.aborts(), Vec::new());

        summaries.insert(summarize(&store));
    }

    // Deterministic result: the finished store is a function of the programs, not of the
    // schedule. One summary across all 1,680 members, and it is not empty.
    assert_eq!(
        summaries.len(),
        1,
        "the outcome varied with the schedule: {summaries:#?}"
    );
    let summary = summaries.into_iter().next().expect("one summary");
    assert!(
        summary.contains("publisher0") && summary.contains("publisher2"),
        "the summary carries no receipts, so the sweep proved nothing: {summary}"
    );
}

// --- sweep 2: visibility is the index write, monotone ---------------------------------------

/// **A reader woven through two distinct publications.** INV-017: an artifact becomes
/// visible at the index commit and never before, and visibility is monotone. The reader's
/// two reads land at every pair of positions in the two publishers' six phases:
/// 8!/(3!·3!·2!) = 560 schedules, and at each one the read's answer is *derived from the
/// schedule* — readable exactly when publisher 0's `CommitIndex` has run.
#[test]
fn positive_a_reader_sees_the_artifact_exactly_from_the_index_commit_on() {
    const WATCHED: &[u8] = b"watched payload";
    const OTHER: &[u8] = b"other payload";
    let lengths = [3, 3, 2];
    let schedules = weavings(&lengths);
    assert_eq!(schedules.len(), multinomial(&lengths));
    assert_eq!(schedules.len(), 560, "8!/(3!·3!·2!)");

    let watched_handle = fnv1a_identity(CLASS, WATCHED);
    let mut readable_observed = 0_usize;
    let mut unreadable_observed = 0_usize;

    for schedule in &schedules {
        let store = store_with(Fnv1aIdentifier, 2);
        let mut slots = [Slot::Idle, Slot::Idle];
        let mut cursors = [0_usize; 3];
        let mut index_committed = false;
        let mut last_read_was_readable = false;

        for &program in schedule {
            if program == 2 {
                // The reader: promoted operator, so a refusal means "not published",
                // never "not authorized".
                let read = store.read(&watched_handle, &mint("operator"));
                assert_eq!(
                    read.is_ok(),
                    index_committed,
                    "visibility disagreed with the schedule: index_committed={index_committed}"
                );
                assert!(
                    !(last_read_was_readable && read.is_err()),
                    "visibility went backwards (INV-017 monotonicity)"
                );
                last_read_was_readable = read.is_ok();
                if read.is_ok() {
                    readable_observed += 1;
                } else {
                    unreadable_observed += 1;
                }
                cursors[2] += 1;
                continue;
            }
            let phase = PUBLISH[cursors[program]];
            cursors[program] += 1;
            let payload = if program == 0 { WATCHED } else { OTHER };
            let slot = std::mem::replace(&mut slots[program], Slot::Idle);
            slots[program] = step(&store, slot, phase, program, payload);
            if program == 0 && phase == Phase::CommitIndex {
                index_committed = true;
            }
        }

        let view = store.audit_view(&mint("operator")).expect("operator");
        assert_eq!(view.published_count(), 2);
        assert_eq!(view.fsck(&Fnv1aIdentifier), Vec::new());
    }

    // Anti-vacuity: the sweep saw both answers, or it swept one behavior.
    assert!(
        readable_observed > 0 && unreadable_observed > 0,
        "the reader saw one answer everywhere (readable={readable_observed}, \
         unreadable={unreadable_observed}), so the monotonicity claim is vacuous"
    );
}

// --- sweep 3: collection vs the in-flight window --------------------------------------------

/// **Garbage collection woven through a live publication.** docs/35's root set: between
/// `commit_content` and `commit_index` the publication is its own root, so collection —
/// wherever the schedule lands it — reclaims nothing, and the publication completes.
/// 4!/(3!·1!) = 4 schedules.
#[test]
fn positive_collection_never_reclaims_a_pinned_publication_at_any_position() {
    const PAYLOAD: &[u8] = b"pinned while in flight";
    let lengths = [3, 1];
    let schedules = weavings(&lengths);
    assert_eq!(schedules.len(), multinomial(&lengths));
    assert_eq!(schedules.len(), 4, "4!/3!");

    let mut collected_between_the_commits = false;
    for schedule in &schedules {
        let store = store_with(Fnv1aIdentifier, 1);
        let mut slot = Slot::Idle;
        let mut cursor = 0_usize;
        for &program in schedule {
            if program == 1 {
                let in_flight_window = matches!(slot, Slot::Committed(_));
                if in_flight_window {
                    // The half-publication *state*, observed live: durable content that no
                    // index entry names. fsck classifies it; collection must still leave it.
                    let view = store.audit_view(&mint("operator")).expect("operator");
                    assert_eq!(
                        view.fsck(&Fnv1aIdentifier),
                        vec![StoreDefect::UnreachableContent(fnv1a_identity(
                            CLASS, PAYLOAD
                        ))],
                        "the in-flight window is unreachable-content residue to fsck"
                    );
                    collected_between_the_commits = true;
                }
                let reclaimed = store
                    .collect_garbage(&mint("operator"))
                    .expect("operator may collect");
                assert_eq!(
                    reclaimed,
                    Vec::new(),
                    "collection reclaimed a live publication's content"
                );
                continue;
            }
            let phase = PUBLISH[cursor];
            cursor += 1;
            slot = step(&store, slot, phase, 0, PAYLOAD);
        }

        // The publication survived collection wherever it landed.
        let receipt = slot.receipt().expect("the publication completed");
        assert_eq!(
            store
                .read(receipt.handle(), &mint("operator"))
                .expect("published"),
            PAYLOAD
        );
        let view = store.audit_view(&mint("operator")).expect("operator");
        assert_eq!(view.fsck(&Fnv1aIdentifier), Vec::new());
    }
    assert!(
        collected_between_the_commits,
        "no schedule put the collection inside the two-commit window, so the pin was never tested"
    );
}

/// **The abandoned half of the same weave.** A publication that commits content and then
/// abandons — the modeled crash between the two commits — is *typed residue*: fsck names
/// it `UnreachableContent`, a final collection reclaims exactly it, and the abort ledger
/// holds the abandonment. 4 schedules again, and the residue answer depends on where the
/// collection landed, which is asserted per schedule rather than averaged away.
#[test]
fn positive_an_abandoned_publication_is_typed_residue_wherever_collection_lands() {
    const PAYLOAD: &[u8] = b"abandoned between the commits";
    let handle = fnv1a_identity(CLASS, PAYLOAD);
    let lengths = [3, 1];
    let schedules = weavings(&lengths);
    assert_eq!(schedules.len(), 4, "4!/3!");

    let mut residue_reclaimed_by_final_collection = 0_usize;
    for schedule in &schedules {
        let store = store_with(Fnv1aIdentifier, 1);
        let mut slot = Slot::Idle;
        let mut cursor = 0_usize;
        let mut abandoned = false;
        for &program in schedule {
            if program == 1 {
                let reclaimed = store
                    .collect_garbage(&mint("operator"))
                    .expect("operator may collect");
                // Before the abandonment the content is either absent or pinned; either
                // way an interleaved collection reclaims nothing.
                assert_eq!(
                    reclaimed,
                    if abandoned {
                        vec![handle.clone()]
                    } else {
                        Vec::new()
                    },
                    "collection disagreed with the schedule"
                );
                continue;
            }
            let phase = ABANDON[cursor];
            cursor += 1;
            slot = step(&store, slot, phase, 0, PAYLOAD);
            if phase == Phase::Abandon {
                abandoned = true;
            }
        }
        assert_eq!(slot.abort_reason(), Some(AbortReason::Abandoned));

        // Nothing was ever visible (INV-017): no index entry, no receipt.
        let view = store.audit_view(&mint("operator")).expect("operator");
        assert_eq!(view.published_count(), 0);
        assert_eq!(view.receipts(&handle), Vec::new());
        assert!(
            view.aborts()
                .iter()
                .any(|abort| abort.reason() == AbortReason::Abandoned),
            "the abandonment left no audit trail"
        );

        // The residue is exactly classified, then exactly reclaimed.
        let residue = store
            .collect_garbage(&mint("operator"))
            .expect("operator may collect");
        if residue == vec![handle.clone()] {
            residue_reclaimed_by_final_collection += 1;
        } else {
            assert_eq!(
                residue,
                Vec::new(),
                "the final collection found something other than the abandoned content"
            );
        }
        assert_eq!(
            store
                .audit_view(&mint("operator"))
                .expect("operator")
                .fsck(&Fnv1aIdentifier),
            Vec::new()
        );
    }
    // Anti-vacuity: in at least one schedule the interleaved collection ran too early and
    // the *final* one had residue to reclaim — otherwise the reclamation path was untested.
    assert!(
        residue_reclaimed_by_final_collection > 0,
        "no schedule left residue for the final collection"
    );
}

// --- sweep 4: byte-different collision, every schedule --------------------------------------

/// **A total identity collision, every schedule.** ADR-0013: the store compares bytes
/// exactly rather than trusting the identity function, so of two byte-different payloads
/// forced onto one identity, whichever `commit_content` lands first wins, the loser
/// aborts `IdentityCollision`, and nothing is ever conflated. 6!/(3!·3!) = 20 schedules,
/// with the winner *derived from the schedule* and asserted per member.
#[test]
fn positive_a_collision_has_one_winner_and_a_typed_loser_at_every_interleaving() {
    const PAYLOADS: [&[u8]; 2] = [b"original", b"impostor"];
    let lengths = [3, 3];
    let schedules = weavings(&lengths);
    assert_eq!(schedules.len(), multinomial(&lengths));
    assert_eq!(schedules.len(), 20, "6!/(3!·3!)");

    let mut winners: BTreeSet<usize> = BTreeSet::new();
    for schedule in &schedules {
        let store = store_with(CollidingIdentifier, 2);
        let mut slots = [Slot::Idle, Slot::Idle];
        let mut cursors = [0_usize; 2];
        let mut first_to_commit_content: Option<usize> = None;

        for &publisher in schedule {
            let phase = PUBLISH[cursors[publisher]];
            cursors[publisher] += 1;
            let slot = std::mem::replace(&mut slots[publisher], Slot::Idle);
            slots[publisher] = step(&store, slot, phase, publisher, PAYLOADS[publisher]);
            if phase == Phase::CommitContent
                && first_to_commit_content.is_none()
                && matches!(slots[publisher], Slot::Committed(_))
            {
                first_to_commit_content = Some(publisher);
            }
        }

        let winner = first_to_commit_content.expect("somebody committed content");
        let loser = 1 - winner;
        winners.insert(winner);

        let receipt = slots[winner].receipt().expect("the winner published");
        assert_eq!(
            slots[loser].abort_reason(),
            Some(AbortReason::IdentityCollision),
            "the loser's refusal is typed, never silent"
        );
        assert_eq!(
            store
                .read(receipt.handle(), &mint("operator"))
                .expect("published"),
            PAYLOADS[winner],
            "the stored bytes are the winner's: nothing was conflated (ADR-0013)"
        );
        let view = store.audit_view(&mint("operator")).expect("operator");
        assert_eq!(view.published_count(), 1);
        assert_eq!(view.receipts(receipt.handle()).len(), 1);
        assert_eq!(view.fsck(&CollidingIdentifier), Vec::new());
    }
    // Anti-vacuity: both publishers won somewhere, or the sweep pinned one order.
    assert_eq!(
        winners,
        BTreeSet::from([0, 1]),
        "one publisher won every schedule, so the collision was never genuinely contested"
    );
}

// --- determinism -----------------------------------------------------------------------------

/// The whole matrix, rendered canonically, is byte-identical across two runs — the
/// repeated-round invariant every campaign in this repository ends on, over bytes because
/// bytes catch an ordering difference that set equality hides.
#[test]
fn positive_the_whole_matrix_renders_byte_identically_across_two_runs() {
    fn render_once() -> String {
        let mut render = String::new();
        for schedule in weavings(&[3, 3]) {
            let store = store_with(Fnv1aIdentifier, 2);
            let mut slots = [Slot::Idle, Slot::Idle];
            let mut cursors = [0_usize; 2];
            for &publisher in &schedule {
                let phase = PUBLISH[cursors[publisher]];
                cursors[publisher] += 1;
                let slot = std::mem::replace(&mut slots[publisher], Slot::Idle);
                slots[publisher] = step(&store, slot, phase, publisher, b"determinism payload");
            }
            render.push_str(&format!("{schedule:?} -> {}\n", summarize(&store)));
        }
        render
    }

    let first = render_once();
    let second = render_once();
    assert_eq!(first.as_bytes(), second.as_bytes());
    assert!(
        first.lines().count() == 20 && first.contains("identities="),
        "the render is not the matrix: {} lines",
        first.lines().count()
    );
}
