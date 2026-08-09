//! **C7 re-measured.** What an enforced `OutputPolicy.max_bytes` is worth on `task.status`,
//! and the ledger projection it falsifies.
//!
//! > | 1 | **C7** `task.status` summary under an enforced `OutputPolicy` | `bn-6fuu5` | 1,114 |
//! > none | the only conformance fix in the set; ships before the bundled minor exists and
//! > needs nothing from the others |
//! >
//! > — `notes/plan/notes/DX10_BYTE_LEDGER.md` §6
//!
//! # What was commissioned, and what was admissible
//!
//! C7's mechanism is stated in §3 of that note: "when the ceiling binds, return the members
//! the caller needs (`task`, `status`, `cost`, `continuation`) and record the remainder as an
//! INV-007 omission whose `recoverable_by` names the task handle", classified
//! **behavior-only, no bump**, worth **27,376 B over the matrix / 1,114 B per solved task**.
//!
//! Returning *those four members and no others* means an answer without `operation`,
//! `snapshot`, `intent`, `epochs`, `priority_class`, `budget`, `milestones` and
//! `committed_evidence`. `TaskRecord` declares six of those `required` and two `nullable`, and
//! `rule versioning.breaking_change` lists "changing a field's type or presence marker" among
//! the changes that MUST advance the **major** and publish a per-artifact-class compatibility
//! statement. So the projected mechanism is not behavior-only, and it is not eligible for the
//! 3.6 *minor* either — a distinction worth more than the bytes, because the whole
//! candidate set was accepted on the promise that item 1 needed no declaration to move.
//! [`the_projected_summary_needs_presence_markers_no_minor_can_move`] states it structurally.
//!
//! What the declaration *does* leave a ceiling is the contents of the two `required` lists and
//! the nine `optional` members of `Budget`. That is what `continuumd::daemon::output` enforces,
//! and [`the_best_available_ceiling_does_not_save_interface_bytes`] measures it:
//! **−1,196 B over the matrix, −49 B per solved task**, taking each poll's *best* ceiling —
//! an optimum no real caller could pick, and still a loss. The reason is exact and is the
//! irony of the item: an INV-007 record carrying `recoverable_by` costs about 136 B, and
//! `recoverable_by`'s value is a fourth copy of the 69-byte task handle the answer already
//! carries three times (§2.2). **Populating the retrieval half — the thing C7 existed to do
//! first — is what makes the trade lose.**
//!
//! # What landed anyway, and why
//!
//! The conformance defect is real independently of bytes: `max_bytes` is declared "the
//! enforced contract" and no handler read it, so a caller that stated one was silently
//! ignored. It is enforced now, with the two typed outcomes
//! `crates/continuumd/src/daemon/output.rs` documents.
//!
//! What did **not** land is a ceiling on the instrument's own polls. The measurement below is
//! why: declaring one would make the typed arm cost more, and adopting a byte change that
//! moves the row the wrong way is no better than adopting one that moves it the right way for
//! the wrong reason. [`the_landed_matrix_is_unmoved_by_this_bone`] is the control.

mod support;

use continuum_benchmark::report::Report;
use continuum_benchmark::surface::Arm;
use continuum_benchmark::variants::{self, CeilingProjection};
use continuum_mcp::{AgentClient, AgentContext, LocalLink};
use continuumd::daemon::family::Payload;
use continuumd::daemon::output::{self, Ceiling, TASK_RECORD_ELISION_ORDER};
use continuumd::protocol::envelope::Omission;
use continuumd::protocol::shared::Target;
use continuumd::protocol::vocabulary::{Encoding, ErrorCode, OmissionReason};

/// The projection, measured once for the file.
fn projection() -> CeilingProjection {
    variants::ceiling_projection().expect("the recording sweep runs")
}

/// **The finding.** Even at each poll's *best* ceiling, an enforced `OutputPolicy` on
/// `task.status` does not save interface bytes on this matrix. It costs them.
///
/// Against `DX10_BYTE_LEDGER.md` §3 C7's projected **+27,376 B / +1,114 B per solved task**.
#[test]
fn the_best_available_ceiling_does_not_save_interface_bytes() {
    let projection = projection();

    assert_eq!(projection.polls, 48, "the matrix's `task.status` answers");
    assert_eq!(
        projection.payload_bytes, 35_368,
        "their payloads, untrimmed"
    );
    assert_eq!(projection.frame_bytes, 66_192, "their frames, untrimmed");

    assert_eq!(
        projection.best_saving, -1_196,
        "the best ceiling available on each poll, summed: a loss, not a saving"
    );
    assert_eq!(projection.best_per_solved(), -49);
    assert!(
        projection.best_saving < 0,
        "and the direction is the finding: C7 projected +27,376 B here"
    );
    assert_eq!(
        projection.floor_saving, -6_128,
        "and asking for the tightest conforming answer — the summary C7 describes — loses \
         five times as much"
    );

    // Where the loss comes from. The declaration costs 34 B on every poll that states it, and
    // the best case only finds a trim worth taking on four polls in forty-eight.
    assert_eq!(projection.declaration_bytes, 1_632);
    assert_eq!(
        projection.declaration_bytes / u64::from(projection.polls),
        34
    );
    assert_eq!(
        projection.trims, 4,
        "on 44 of 48 polls the cheapest thing a stated ceiling can do is take nothing"
    );

    // The record that makes it lose, priced. `recoverable_by` is a fourth copy of a handle
    // the answer already carries three times (§2.2), and it is INV-007's retrieval half —
    // which is the half C7 existed to populate for the first time on this wire.
    let with_route = omission_bytes(true);
    let without = omission_bytes(false);
    assert!(
        (130..=140).contains(&with_route),
        "an INV-007 record naming its route costs about 136 B, measured {with_route}"
    );
    assert!(
        with_route > without * 2,
        "and the route is most of it: {without} B without, {with_route} B with"
    );
}

/// **The declaration finding, structurally.** The mechanism C7 projected cannot be built out
/// of values `TaskRecord` admits: the six members whose elision produced its saving are
/// `required` or `nullable`, and a ceiling therefore cannot take them however tight it is.
#[test]
fn the_projected_summary_needs_presence_markers_no_minor_can_move() {
    let (record, _) = a_polled_record();

    // The floor: the whole declared elision order taken, which is the smallest conforming
    // answer this operation has.
    let mut floor = record.clone();
    for subject in TASK_RECORD_ELISION_ORDER {
        let mut probe = floor.clone();
        take(&mut probe, subject);
        floor = probe;
    }
    let ceiling = output::measure(&floor, Encoding::CanonicalJson).expect("measurable");
    let fitted = output::fit_task_record(
        record.clone(),
        Ceiling::of_bytes(ceiling),
        Encoding::CanonicalJson,
    )
    .expect("the floor is an answer");

    // Everything C7's summary drops, still on the wire — because the declaration says so.
    assert_eq!(fitted.record.operation, record.operation, "`required`");
    assert_eq!(fitted.record.snapshot, record.snapshot, "`nullable`");
    assert_eq!(fitted.record.intent, record.intent, "`nullable`");
    assert_eq!(
        fitted.record.epochs, record.epochs,
        "`required`, and a trim"
    );
    assert_eq!(
        fitted.record.priority_class, record.priority_class,
        "`required`"
    );

    // And the elision order is exactly the three places a smaller value is still a conforming
    // value. A fourth would be a presence marker moving.
    assert_eq!(
        TASK_RECORD_ELISION_ORDER,
        ["task.milestones", "task.committed_evidence", "task.budget"]
    );
}

/// **End to end, through the typed client.** The enforcement is reachable from the ACI, the
/// summary is readable, and the manifest hands back the route.
#[test]
fn a_bounded_poll_comes_back_summarized_with_its_retrieval_route() {
    let (record, mut session) = a_polled_record();
    let whole = output::measure(&record, Encoding::CanonicalJson).expect("measurable");

    let answer = session.poll_bounded(whole - 1);
    let admitted = answer.success().expect("a bounded poll is admitted");
    let Payload::TaskStatus(summary) = &admitted.payload else {
        panic!("a task.status answer")
    };
    assert!(summary.milestones.is_empty(), "the summary is smaller");
    assert_eq!(summary.task, record.task, "and it is about the same task");
    assert_eq!(summary.status, record.status);
    assert_eq!(summary.cost, record.cost);
    assert_eq!(summary.epochs, record.epochs, "no epoch was dropped");

    let trims: Vec<&Omission> = admitted
        .omissions
        .iter()
        .filter(|omission| omission.reason == OmissionReason::Budget)
        .collect();
    assert_eq!(trims.len(), 1);
    assert_eq!(trims[0].subject, "task.milestones");
    assert_eq!(
        trims[0]
            .recoverable_by
            .value()
            .map(|handle| handle.as_str()),
        Some(record.task.as_str()),
        "INV-007's retrieval half, populated for the first time on this wire"
    );

    // INV-009: the route recovers what the ceiling took, and the re-read is a superset.
    let again = session.poll();
    let Payload::TaskStatus(expanded) = &again.success().expect("admitted").payload else {
        panic!("a task.status answer")
    };
    assert_eq!(expanded, &record, "the whole record, unchanged");
    assert!(expanded.milestones.len() > summary.milestones.len());
}

/// **Over the bound.** A ceiling nothing conforming fits is a typed refusal, not a payload
/// over the ceiling — read here at the interface a client actually has.
#[test]
fn a_ceiling_below_the_floor_reaches_the_client_as_a_typed_refusal() {
    let (_, mut session) = a_polled_record();
    let answer = session.poll_bounded(1);
    assert!(answer.success().is_none(), "a refusal is not an answer");
    assert_eq!(answer.error_code(), Some(ErrorCode::MalformedRequest));
    assert!(
        !answer.admitted(),
        "and it is counted as a refused attempt, like every other typed refusal"
    );
}

/// **The control.** The landed matrix is byte-for-byte what it was: the instrument states no
/// ceiling, so the daemon measures nothing and answers exactly as before.
#[test]
fn the_landed_matrix_is_unmoved_by_this_bone() {
    let report = Report::new(support::both_arms());
    let native = report.totals[&Arm::Native];
    let shell = report.totals[&Arm::Shell];

    // 10,384 as of bn-3of5h, not 10,859: `workspace.create_by_reference` landed at
    // protocol 3.6 and the typed arm takes it. Nothing in *this* bone moved it — the
    // control's claim is unchanged and is still checked against the matrix, at the value
    // the matrix now has.
    assert_eq!(native.bytes_per_solved(), Some(10_384));
    assert_eq!(shell.bytes_per_solved(), Some(4_118));
    assert_eq!((native.solved, shell.solved), (24, 24));
    assert_eq!(native.invalid_permille(), 69);
    assert_eq!(shell.invalid_permille(), 69);
}

// --- fixtures -------------------------------------------------------------------------------

/// One `task.status` answer, and the live session that produced it.
struct Session {
    rig: continuum_benchmark::rig::Rig,
    client: AgentClient,
    context: AgentContext,
    task: continuumd::protocol::scalar::TaskHandle,
}

impl Session {
    fn poll(&mut self) -> continuum_mcp::answer::Answer {
        let context = self.context;
        let (server, pair, welcome) = self.rig.boundary();
        let mut link = LocalLink::new(server, pair, welcome);
        self.client
            .task_status(&mut link, &context, &self.task)
            .expect("the poll reaches the daemon")
    }

    fn poll_bounded(&mut self, max_bytes: u64) -> continuum_mcp::answer::Answer {
        let context = self.context;
        let (server, pair, welcome) = self.rig.boundary();
        let mut link = LocalLink::new(server, pair, welcome);
        self.client
            .task_status_bounded(&mut link, &context, &self.task, max_bytes)
            .expect("the poll reaches the daemon")
    }
}

/// A campaign, run to a record with something in it a ceiling could take.
///
/// The script is the faithful policy's own opening — create-and-seal, start, poll — driven
/// through `AgentClient` directly rather than through `support::NativeSurface`, because these
/// tests need the *session* afterwards and a `Surface` hands back only its observation. The
/// `AgentContext` is threaded exactly as that surface threads it, so the client's own local
/// refusals fire here the way they fire in a run.
fn a_polled_record() -> (continuumd::protocol::task::TaskRecord, Session) {
    use continuum_benchmark::rig::{Principal, Rig};
    use continuum_benchmark::task::DIE_HARD_ALL;

    let mut rig = Rig::fresh();
    let mut client = AgentClient::new(
        continuum_benchmark::rig::version(),
        Principal::BUILDER.actor_id(),
        Principal::BUILDER.capability_handle(),
    );
    let mut context = AgentContext::EMPTY;
    let components = rig.components(DIE_HARD_ALL.source);
    let hello = continuum_benchmark::rig::hello();
    {
        let (server, pair, welcome) = rig.boundary();
        let mut link = LocalLink::new(server, pair, welcome);
        client
            .open(&mut link, &hello)
            .expect("the connection opens");
    }

    let snapshot = {
        let (server, pair, welcome) = rig.boundary();
        let mut link = LocalLink::new(server, pair, welcome);
        let answer = client
            .workspace_create(&mut link, &context, components, true)
            .expect("the create reaches the daemon");
        let admitted = answer.success().expect("the create is admitted");
        context.observe("workspace.create", admitted);
        let Payload::WorkspaceCreate(response) = &admitted.payload else {
            panic!("a workspace.create answer")
        };
        response.snapshot.clone()
    };

    client.speak_as(
        Principal::RUNNER.actor_id(),
        Principal::RUNNER.capability_handle(),
    );
    let task = {
        let (server, pair, welcome) = rig.boundary();
        let mut link = LocalLink::new(server, pair, welcome);
        let answer = client
            .verification_start(
                &mut link,
                &context,
                &snapshot,
                Target {
                    kind: DIE_HARD_ALL.target_kind,
                    id: DIE_HARD_ALL.target_id.to_owned(),
                },
                continuum_benchmark::declared_budget(64),
            )
            .expect("the start reaches the daemon");
        let admitted = answer.success().expect("the start is admitted");
        let task = admitted
            .task
            .clone()
            .expect("a started campaign names its task");
        context.observe("verification.start", admitted);
        task
    };

    client.speak_as(
        Principal::READER.actor_id(),
        Principal::READER.capability_handle(),
    );
    let mut session = Session {
        rig,
        client,
        context,
        task,
    };
    let answer = session.poll();
    let Payload::TaskStatus(record) = &answer.success().expect("the poll is admitted").payload
    else {
        panic!("a task.status answer")
    };
    let record = record.clone();
    (record, session)
}

/// The measured size of one INV-007 trim record, with and without its retrieval route.
fn omission_bytes(with_route: bool) -> u64 {
    use continuumd::protocol::scalar::ArtifactHandle;
    use continuumd::protocol::spec::Optional;

    let omission = Omission {
        reason: OmissionReason::Budget,
        subject: "task.milestones".to_owned(),
        recoverable_by: if with_route {
            Optional::Present(
                ArtifactHandle::new(&format!("task_{}", "0".repeat(64))).expect("a handle"),
            )
        } else {
            Optional::Absent
        },
    };
    // Plus the separator a list member costs beside its neighbours.
    output::measure(&omission, Encoding::CanonicalJson).expect("measurable") + 1
}

/// The elision the daemon's declared order names, used here only to choose a ceiling to ask
/// for; every answer is still built by the daemon's own `fit_task_record`.
fn take(record: &mut continuumd::protocol::task::TaskRecord, subject: &str) {
    use continuumd::protocol::envelope::Budget;
    use continuumd::protocol::spec::Optional;

    match subject {
        "task.milestones" => record.milestones = Vec::new(),
        "task.committed_evidence" => record.committed_evidence = Vec::new(),
        "task.budget" => {
            record.budget = Budget {
                wall_ms: Optional::Absent,
                cpu_ms: Optional::Absent,
                memory_bytes: Optional::Absent,
                states: Optional::Absent,
                solver_ms: Optional::Absent,
                proof_ms: Optional::Absent,
                tokens: Optional::Absent,
                candidates: Optional::Absent,
                bytes: Optional::Absent,
            };
        }
        _ => {}
    }
}
