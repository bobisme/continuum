//! The Phase A exit's triple-surface check (plan §21, bn-id4).
//!
//! > Exit: Die Hard and Dining Philosophers can be checked through native API, CLI, and an
//! > agent client with identical artifacts […]
//! >
//! > — plan §21, Phase A
//!
//! Cited verbatim from `notes/plan/plan.md` by [`the_exit_sentence_this_file_answers_is_still_the_plans`],
//! so a reworded exit fails here rather than leaving this file answering a sentence that
//! moved. Plan §21.1 makes the obligation permanent — "A gate's criteria remain permanent
//! regressions after closure (§22 release-blocker doctrine)" — which is why this is a test
//! and not a report.
//!
//! # What "identical artifacts" is read to mean, and what it is not
//!
//! An *artifact* here is what a check **produced**: a content-addressed, durably published
//! record, plus the receipt the store issued for it. It is not a rendering. The claim under
//! test is that the three surfaces are projections of one truth rather than three
//! implementations, so the check is **content-addressed equality plus byte equality**:
//!
//! - the content addresses agree (`ArtifactHandle`, `Commitment`), and
//! - the bytes those addresses name agree, read back out of each deployment's own
//!   `ReferenceStore`, and
//! - the publication receipts agree, and
//! - the canonical wire encoding of the typed answer agrees.
//!
//! Equal verdicts would not have been enough, and equal hashes alone would not have been
//! either: [`every_published_address_is_the_blake3_identity_of_its_own_bytes`] binds the two
//! halves together, so "the hashes matched" cannot be true of two empty or two mismatched
//! stores.
//!
//! # The three surfaces, and the reading each name was given
//!
//! | Plan §21 name | Read as | Driven through |
//! |---|---|---|
//! | native API | `continuumd`'s own typed operation layer | [`Daemon::dispatch`] — a typed `OperationRequest` in, a typed `OperationOutcome` out, no adapter and no codec |
//! | CLI | `continuum-cli` | [`continuum_cli::cli::run`] — real argv, the real `Scan`, a real frame; and `continuum_cli::wire::Connection`, which is what every command funnels through |
//! | agent client | `continuum-mcp` | [`AgentClient`]'s named operations over `continuum_mcp::LocalLink` |
//!
//! The third name is the one that needed deciding, and the honest reading is recorded rather
//! than assumed. `continuum-mcp`'s own crate documentation claims this exact sentence —
//! "plan §20's crate list names exactly one agent-facing client surface — this one — and the
//! Phase A exit sentence names it as a third surface beside the native API and the CLI" —
//! and plan §24.5's DX-10 fallback row names "MCP-only surface", so the surface the exit
//! grades is the one in that crate. `continuum-benchmark`'s own `tests/support`
//! `NativeSurface` is a *driver of* that client, not a second one, and
//! `continuum_benchmark::shell` is explicitly a text-projection **simulacrum** of a CLI, not
//! `continuum-cli` — which is why the surface driven here is the real crate.
//! [`the_agent_client_reading_is_the_one_its_own_crate_claims`] pins the reading against the
//! text that grounds it.
//!
//! # Why this file lives in `continuum-benchmark/tests/`
//!
//! Three reasons, in order of weight.
//!
//! 1. **INV-005.** The check is only meaningful if the three surfaces are driven with
//!    *identical explicit inputs*, and the largest input is the provisioned deployment.
//!    [`Rig::fresh`] is the one landed thing in this workspace that stages **both** corpus
//!    ports, registers both models, and accepts one governing intent, in one deterministic
//!    function. Re-provisioning that by hand somewhere else would put the very thing under
//!    test — sameness of inputs — into hand-written fixture code three times over.
//! 2. **The dependency graph.** The file needs `continuum-cli` and `continuum-mcp`, and both
//!    are **adapters**: plan §20's "adapters do not own semantic state" is enforced as
//!    "adapters are sinks in the workspace graph" (`tools/check_crate_boundaries.py`, RULE
//!    adapters-are-sinks). `continuum-benchmark` is the one crate whose manifest already
//!    documents and takes that dev-dependency, by that checker's own documented design.
//!    Putting the file in `continuumd/tests/` would have spelled `continuumd ->
//!    continuum-cli`, which is verbatim the edge that checker's self-test injects as its
//!    canonical violation.
//! 3. **What this crate is.** "may depend on Forge and on verifier interfaces — it is a
//!    harness, not part of the verifier" (`src/lib.rs`, from plan §20). A differential test
//!    across three surfaces is harness work.
//!
//! # Both fixtures, and what is absent
//!
//! Both named models are driven end to end: Die Hard (TV-009) and Dining Philosophers
//! (TV-007), as [`DIE_HARD_ALL`] and [`PHILOSOPHERS_ALL`]. Neither is a frozen answer being
//! echoed — each campaign is a live `bfs::explore` inside `continuum-engine-reference`, and
//! the frozen state counts are asserted as *oracles* against what the wire returned.
//!
//! Four absences are typed rather than skipped, each with a tripwire that fails when the
//! absence is filled — see [`the_absences_this_check_runs_around_are_still_absent`]:
//!
//! - **No Dining Philosophers Intent Contract fixture.** `continuum_benchmark::corpus`
//!   deliberately runs both ports under the Die Hard contract.
//! - **No certificate for Dining Philosophers.** Certificate emission exists and is
//!   exercised for Die Hard only.
//! - **No crashpack producer at all**, so the depth-6 solution and the depth-10 deadlock have
//!   a wire home nothing fills.
//! - **No transition count or witness depth on the wire.** `Cost` has nine dimensions and
//!   none is a transition count (RFC 0026 F16).
//!
//! # Clause → test
//!
//! - **the exit sentence, verbatim, still says this** →
//!   [`the_exit_sentence_this_file_answers_is_still_the_plans`]
//! - **identical inputs, checked rather than assumed** →
//!   [`the_three_surfaces_declare_one_budget_and_one_request_shape`]
//! - **byte-identical artifacts and receipts, Die Hard** →
//!   [`die_hard_publishes_byte_identical_artifacts_through_all_three_surfaces`]
//! - **byte-identical artifacts and receipts, Dining Philosophers** →
//!   [`dining_philosophers_publishes_byte_identical_artifacts_through_all_three_surfaces`]
//! - **byte-identical typed answers where a surface returns one** →
//!   [`the_typed_verification_result_is_byte_identical_across_all_three_surfaces`]
//! - **the CLI's rendering is a projection and names no other artifact** →
//!   [`the_cli_rendering_projects_the_same_content_addresses_and_invents_none`]
//! - **anti-vacuity: the addresses really do name the bytes** →
//!   [`every_published_address_is_the_blake3_identity_of_its_own_bytes`]
//! - **anti-vacuity: equality is a constraint, not a constant** →
//!   [`one_changed_input_changes_the_artifacts_so_the_equality_above_is_a_constraint`]
//! - **the recorded reading of "agent client"** →
//!   [`the_agent_client_reading_is_the_one_its_own_crate_claims`]
//! - **typed absences, with tripwires** →
//!   [`the_absences_this_check_runs_around_are_still_absent`]
//!
//! [`Daemon::dispatch`]: continuumd::daemon::Daemon::dispatch
//! [`Rig::fresh`]: continuum_benchmark::rig::Rig::fresh
//! [`DIE_HARD_ALL`]: continuum_benchmark::task::DIE_HARD_ALL
//! [`PHILOSOPHERS_ALL`]: continuum_benchmark::task::PHILOSOPHERS_ALL

use std::collections::BTreeMap;

use continuum_benchmark::rig::{Principal, Rig};
use continuum_benchmark::task::{BenchmarkTask, DIE_HARD_ALL, PHILOSOPHERS_ALL};
use continuum_cli::wire::{Connection, LocalLink as CliLink, Outcome as CliOutcome};
use continuum_mcp::{AgentClient, AgentContext, LocalLink as AgentLink};
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{ContentIdentifier, PublicationReceipt};
use continuumd::codec::json::Json;
use continuumd::codec::operations::encode_payload;
use continuumd::codec::{ProtocolValue, to_bytes};
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::{Blake3Identity, capability_to_store};
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{
    ArtifactRef, AssuranceEnvelope, Budget, Cost, Omission, RequestEnvelope, Verdict,
};
use continuumd::protocol::operations::task::TaskStatusRequest;
use continuumd::protocol::operations::verification::{
    VerificationResultRequest, VerificationStartRequest, VerificationStartResponse,
};
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::scalar::{Opaque, OperationName, RequestId, TaskHandle, WorkspaceHandle};
use continuumd::protocol::shared::Target;
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{Portfolio, ResultStatus, SemanticVerdict};

// --- the cited texts, all tracked in this repository ----------------------------------------

const PLAN_MD: &str = include_str!("../../../notes/plan/plan.md");
const MCP_LIB: &str = include_str!("../../continuum-mcp/src/lib.rs");
const BENCHMARK_SHELL: &str = include_str!("../src/shell.rs");
const CORPUS: &str = include_str!("../src/corpus.rs");
const CLI_MANIFEST: &str = include_str!("../../continuum-cli/Cargo.toml");
const MCP_TYPED_SURFACE: &str = include_str!("../../continuum-mcp/tests/typed_surface.rs");
const CLI_CHECK_GOLDEN: &str = include_str!(
    "../../continuum-cli/tests/golden/pr13-03-check-result--die-hard-refuted.json.golden"
);

/// The Phase A exit sentence, verbatim from `notes/plan/plan.md`.
const EXIT_SENTENCE: &str = "Exit: Die Hard and Dining Philosophers can be checked through native API,\nCLI, and an agent client with identical artifacts;";

/// Plan §21.1's rule that closes the "is this still owed?" question.
const REGRESSION_RULE: &str = "A gate's criteria remain permanent regressions after\nclosure";

// --- the three surfaces ---------------------------------------------------------------------

/// One of plan §21's three named surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Surface {
    /// `continuumd`'s own typed operation layer, called directly.
    NativeApi,
    /// `continuum-cli`, driven from argv.
    Cli,
    /// `continuum-mcp`'s `AgentClient`.
    AgentClient,
}

impl Surface {
    /// All three, in the order plan §21 names them.
    const ALL: [Self; 3] = [Self::NativeApi, Self::Cli, Self::AgentClient];

    /// The stable token a failure names this surface by.
    const fn token(self) -> &'static str {
        match self {
            Self::NativeApi => "native-api",
            Self::Cli => "cli",
            Self::AgentClient => "agent-client",
        }
    }
}

// --- what one surface's run produced ---------------------------------------------------------

/// One answered operation, in the protocol's own types, however the surface spelled it.
///
/// The two envelope fields deliberately *not* here are `request_id` and the request's
/// idempotency key. Each adapter mints its own — `continuum-cli` counts `req_cli{n}`,
/// `continuum-mcp` counts from its own call counter, and the native arm names its own — and
/// RFC 0026 makes neither a part of any artifact identity. Holding them equal would have
/// hidden exactly what this file is for: that the artifacts do not depend on who asked.
#[derive(Debug, Clone)]
struct Answered {
    status: ResultStatus,
    payload: Payload,
    verdict: Optional<Verdict>,
    assurance: Optional<AssuranceEnvelope>,
    artifacts: Vec<ArtifactRef>,
    cost: Cost,
    omissions: Vec<Omission>,
}

impl Answered {
    /// The whole answer, as one canonical-JSON document.
    ///
    /// Built through `continuumd`'s own encoders — `codec::operations::encode_payload` for the
    /// body and `codec::to_bytes` for each envelope field — so this is
    /// `rule encoding.canonical_form`'s bytes and not a second spelling of them.
    fn canonical_bytes(&self) -> Vec<u8> {
        let payload = encode_payload(&self.payload)
            .expect("every payload this file reads is encodable")
            .map_or(Json::Null, |opaque| {
                Json::parse(opaque.as_bytes()).expect("an encoded payload is canonical JSON")
            });
        Json::object([
            (
                "status".to_owned(),
                Json::String(self.status.as_wire().to_owned()),
            ),
            ("payload".to_owned(), payload),
            ("verdict".to_owned(), optional_json(self.verdict.value())),
            (
                "assurance".to_owned(),
                optional_json(self.assurance.value()),
            ),
            (
                "artifacts".to_owned(),
                Json::Array(self.artifacts.iter().map(as_json).collect()),
            ),
            ("cost".to_owned(), as_json(&self.cost)),
            (
                "omissions".to_owned(),
                Json::Array(self.omissions.iter().map(as_json).collect()),
            ),
        ])
        .expect("seven distinct keys")
        .to_canonical_bytes()
    }
}

/// One protocol value as its canonical-JSON document.
fn as_json<T: ProtocolValue>(value: &T) -> Json {
    Json::parse(&to_bytes(value).expect("every protocol value this file encodes is encodable"))
        .expect("a canonical encoding parses back")
}

/// An absent optional as a *named* absence (INV-007), never an omission.
fn optional_json<T: ProtocolValue>(value: Option<&T>) -> Json {
    value.map_or(Json::Null, as_json)
}

/// Everything one surface's run produced, in the forms the exit sentence names.
///
/// Deliberately *not* a rendering of any surface's own vocabulary: every field here is read
/// through one of two seams that exist below all three surfaces — the deployment's
/// `ReferenceStore`, and `continuumd`'s canonical encoding. That is what makes comparing them
/// a statement about the surfaces rather than about this file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Produced {
    /// The sealed snapshot the campaign ran over — a content identity.
    snapshot: WorkspaceHandle,
    /// The campaign — a content identity derived from the request.
    task: TaskHandle,
    /// Every artifact the deployment published: content address, then the bytes it names.
    published: Vec<(ArtifactHandle, Vec<u8>)>,
    /// Every publication receipt the store issued, in ledger order.
    receipts: Vec<PublicationReceipt>,
    /// Committed bytes per artifact class.
    attribution: BTreeMap<ArtifactClass, u64>,
    /// The canonical bytes of each answered operation, in script order.
    answers: Vec<(&'static str, Vec<u8>)>,
    /// The content-addressed refs the `verification.start` answer named.
    start_refs: Vec<ArtifactRef>,
    /// The folded semantic verdict, kept unencoded so an oracle can read it.
    verdict: SemanticVerdict,
    /// The reachable-state count the wire reported on `task.status`.
    states: Optional<u64>,
}

/// The four answers one surface's script produced, in script order.
struct Script {
    snapshot: WorkspaceHandle,
    task: TaskHandle,
    created: Answered,
    started: Answered,
    status: Answered,
    result: Answered,
}

// --- the one script, and the inputs every surface declares ------------------------------------

/// The budget every surface declares, in the one shape all three can spell.
///
/// `continuum_cli::task::states_budget` and `continuum_mcp::client::states_budget` are two
/// declarations of one value, and
/// [`the_three_surfaces_declare_one_budget_and_one_request_shape`] checks they still agree
/// rather than trusting that they do. The CLI's `check start` reaches this shape and no other
/// — `StartArgs::budget()` is `states_budget(self.states)` — so a triple-surface check that
/// declared a wall or byte ceiling on the other two arms would have been comparing three runs
/// of two different campaigns: the campaign identity is a function of the budget
/// (`daemon::verification::start_handle`), which
/// [`one_changed_input_changes_the_artifacts_so_the_equality_above_is_a_constraint`] shows.
fn budget(task: &BenchmarkTask) -> Budget {
    continuum_cli::task::states_budget(task.ceiling)
}

/// What every surface aims the campaign at.
fn target(task: &BenchmarkTask) -> Target {
    Target {
        kind: task.target_kind,
        id: task.target_id.to_owned(),
    }
}

/// The `workspace.create` request every surface sends, in the one shape all three build.
fn create_request(rig: &Rig, task: &BenchmarkTask) -> WorkspaceCreateRequest {
    WorkspaceCreateRequest {
        components: rig.components(task.source),
        // Empty means "the caller declared none", which travels as `Absent` on all three
        // surfaces: `continuum_cli::snapshot::CreateArgs::request` and
        // `AgentClient::workspace_create` agree, and this restates their agreement.
        overlay: Optional::Absent,
        seal: Optional::Present(true),
    }
}

// --- reading a deployment, identically for every surface ---------------------------------------

/// The store token `cap_root` crosses to.
fn operator() -> continuum_workspace::publication::CapabilityToken {
    capability_to_store(&Principal::ROOT.capability_handle())
        .expect("`cap_root` is a well-formed store token")
}

/// What one deployment's publication store holds, once a surface has driven it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stored {
    /// Every artifact published: content address, then the bytes it names.
    published: Vec<(ArtifactHandle, Vec<u8>)>,
    /// Every publication receipt the store issued, in ledger order.
    receipts: Vec<PublicationReceipt>,
    /// Committed bytes per artifact class.
    attribution: BTreeMap<ArtifactClass, u64>,
}

/// Read one deployment's store through the operator view.
///
/// This is the seam the whole file rests on, and it is the *same* function for all three
/// surfaces: no surface gets to describe its own artifacts.
fn read_store(daemon: &Daemon) -> Stored {
    let operator = operator();
    let audit = daemon
        .store()
        .audit_view(&operator)
        .expect("`cap_root` confers audit");
    let mut published = Vec::new();
    let mut receipts = Vec::new();
    for handle in audit.identities() {
        let bytes = daemon
            .store()
            .read(&handle, &operator)
            .expect("an indexed identity reads back under the operator token");
        receipts.extend(audit.receipts(&handle));
        published.push((handle, bytes));
    }
    assert!(
        audit.fsck(&Blake3Identity).is_empty(),
        "a deployment this check reads must have no store defects"
    );
    Stored {
        published,
        receipts,
        attribution: audit.storage_attribution(),
    }
}

/// Fold one surface's script and its deployment into the comparable record.
fn produced(rig: &Rig, script: Script) -> Produced {
    assert_eq!(
        script.result.status,
        ResultStatus::Ok,
        "a closed campaign's result is answered `ok`"
    );
    let Payload::VerificationResult(result) = &script.result.payload else {
        panic!("`verification.result` answers a `VerificationResult` payload");
    };
    assert_eq!(
        result.task, script.task,
        "the result is about the task that ran"
    );
    let Some(Verdict::Semantic(semantic)) = script.result.verdict.value() else {
        panic!("a verification result carries a semantic verdict");
    };
    assert!(
        script.result.assurance.value().is_some(),
        "`rule envelope.assurance_required` puts an assurance envelope on every semantic verdict"
    );
    let Payload::TaskStatus(record) = &script.status.payload else {
        panic!("`task.status` answers a task record");
    };
    let stored = read_store(rig.daemon());
    Produced {
        snapshot: script.snapshot,
        task: script.task,
        published: stored.published,
        receipts: stored.receipts,
        attribution: stored.attribution,
        answers: vec![
            ("workspace.create", script.created.canonical_bytes()),
            ("verification.start", script.started.canonical_bytes()),
            ("task.status", script.status.canonical_bytes()),
            ("verification.result", script.result.canonical_bytes()),
        ],
        start_refs: script.started.artifacts.clone(),
        verdict: semantic.verdict,
        states: record.cost.states,
    }
}

// --- surface 1: the native API -----------------------------------------------------------------

/// One request envelope, at the negotiated version, for the native arm.
fn native_envelope(
    rig: &Rig,
    operation: &str,
    principal: Principal,
    request: &str,
    key: Option<&str>,
    snapshot: Nullable<WorkspaceHandle>,
    budget: Optional<Budget>,
) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: rig.version(),
        request_id: RequestId::new(request).expect("a well-formed request id"),
        idempotency_key: key.map_or(Optional::Absent, |key| Optional::Present(key.to_owned())),
        actor: principal.actor_id(),
        capability: principal.capability_handle(),
        operation: OperationName::new(operation).expect("a registry name is well formed"),
        snapshot,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

/// One `Daemon::dispatch` call, folded into the shape every surface reports.
fn dispatch(rig: &mut Rig, envelope: RequestEnvelope, arguments: Arguments) -> Answered {
    let outcome = rig.boundary().0.daemon_mut().dispatch(&OperationRequest {
        envelope,
        arguments,
    });
    assert!(
        outcome.envelope.error.value().is_none(),
        "the native arm was refused: {:?}",
        outcome.envelope.error
    );
    Answered {
        status: outcome.envelope.status,
        payload: outcome.payload,
        verdict: match outcome.envelope.verdict {
            Nullable::Value(verdict) => Optional::Present(verdict),
            Nullable::Null => Optional::Absent,
        },
        assurance: outcome.envelope.assurance,
        artifacts: outcome.envelope.artifacts,
        cost: outcome.envelope.cost,
        omissions: outcome.envelope.omissions,
    }
}

/// Drive the whole lane through `Daemon::dispatch` — typed in, typed out, no adapter and no
/// codec between the caller and the operation layer.
fn drive_native(rig: &mut Rig, task: &BenchmarkTask, budget: Budget) -> Produced {
    let create = create_request(rig, task);
    let envelope = native_envelope(
        rig,
        "workspace.create",
        Principal::BUILDER,
        "req_native_create",
        Some("native-create"),
        Nullable::Null,
        Optional::Absent,
    );
    let created = dispatch(rig, envelope, Arguments::WorkspaceCreate(create));
    let Payload::WorkspaceCreate(response) = &created.payload else {
        panic!("a create answers a create payload");
    };
    let snapshot = response.snapshot.clone();

    let envelope = native_envelope(
        rig,
        "verification.start",
        Principal::RUNNER,
        "req_native_start",
        Some("native-start"),
        Nullable::Value(snapshot.clone()),
        Optional::Present(budget),
    );
    let started = dispatch(
        rig,
        envelope,
        Arguments::VerificationStart(VerificationStartRequest {
            target: target(task),
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    );
    let handle = started_task(&started);

    let envelope = native_envelope(
        rig,
        "task.status",
        Principal::RUNNER,
        "req_native_status",
        None,
        Nullable::Null,
        Optional::Absent,
    );
    let status = dispatch(
        rig,
        envelope,
        Arguments::TaskStatus(TaskStatusRequest {
            task: handle.clone(),
        }),
    );

    let envelope = native_envelope(
        rig,
        "verification.result",
        Principal::RUNNER,
        "req_native_result",
        None,
        Nullable::Null,
        Optional::Absent,
    );
    let result = dispatch(
        rig,
        envelope,
        Arguments::VerificationResult(VerificationResultRequest {
            task: handle.clone(),
        }),
    );

    produced(
        rig,
        Script {
            snapshot,
            task: handle,
            created,
            started,
            status,
            result,
        },
    )
}

/// The task handle a `task_started` answer names.
fn started_task(started: &Answered) -> TaskHandle {
    assert_eq!(
        started.status,
        ResultStatus::TaskStarted,
        "a fresh campaign is task-shaped"
    );
    let Payload::VerificationStart(VerificationStartResponse { task, .. }) = &started.payload
    else {
        panic!("a start answers a start payload");
    };
    task.value()
        .cloned()
        .expect("a task-shaped answer names its task")
}

// --- surface 2: the CLI -------------------------------------------------------------------------

/// Build one argv: the command's own words, then the connection flags every invocation
/// carries.
///
/// `--protocol-version` is passed explicitly and is *not* a convenience. `continuum-cli`'s
/// `DEFAULT_PROTOCOL_VERSION` is `(3, 2)` while this deployment negotiates
/// `continuum_benchmark::rig::VERSION`, and RFC 0026 admits a request only at the negotiated
/// version. [`the_absences_this_check_runs_around_are_still_absent`] pins that gap so the flag
/// stops being needed the day the default catches up, rather than quietly staying.
fn argv(rig: &Rig, words: &[&str], who: Principal) -> Vec<String> {
    let version = rig.version();
    let mut args: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
    for flag in [
        "--actor",
        who.actor,
        "--capability",
        who.capability,
        "--format",
        "json",
        "--protocol-version",
        &format!("{}.{}", version.major(), version.minor()),
    ] {
        args.push(flag.to_owned());
    }
    args
}

/// Run one invocation through `continuum_cli::cli::run` — the outermost seam: real argv, the
/// real `Scan`, a real frame.
fn invoke(rig: &mut Rig, args: &[String]) -> Json {
    let (server, pair, _) = rig.boundary();
    let mut link = CliLink::new(server, pair);
    let rendered = continuum_cli::cli::run(args, &mut link, false)
        .expect("the call reaches a real frame and a real answer");
    Json::parse(rendered.text.trim_end().as_bytes())
        .expect("`--format json` renders one canonical JSON document")
}

/// One string field of a rendered answer.
fn token(rendering: &Json, key: &str) -> String {
    field(rendering, key)
        .as_str()
        .unwrap_or_else(|| panic!("{key:?} is not a string in the rendering"))
        .to_owned()
}

/// One field of a rendered answer.
fn field<'a>(rendering: &'a Json, key: &str) -> &'a Json {
    rendering
        .as_object()
        .expect("a rendered answer is an object")
        .get(key)
        .unwrap_or_else(|| panic!("no {key:?} field in the rendering"))
}

/// One typed answer through `continuum-cli`'s own `Connection`.
fn connect(rig: &mut Rig, connection: &mut Connection, call: CliCall) -> Answered {
    let outcome = {
        let (server, pair, _) = rig.boundary();
        let mut link = CliLink::new(server, pair);
        match call {
            CliCall::Status(task) => connection.task_status(&mut link, &task),
            CliCall::Result(request) => connection.verification_result(&mut link, &request),
        }
        .expect("the call reaches a real frame")
    };
    let CliOutcome::Admitted(admitted) = outcome else {
        panic!("the CLI's connection was refused an answer the other surfaces were given");
    };
    Answered {
        status: admitted.status,
        payload: admitted.payload,
        verdict: admitted.verdict.map_or(Optional::Absent, Optional::Present),
        assurance: admitted
            .assurance
            .map_or(Optional::Absent, Optional::Present),
        artifacts: admitted.artifacts,
        cost: admitted.cost,
        omissions: admitted.omissions,
    }
}

/// The two read calls the CLI arm makes through its own `Connection`.
enum CliCall {
    Status(TaskHandle),
    Result(VerificationResultRequest),
}

/// What one CLI run rendered, alongside what it produced.
struct CliRun {
    produced: Produced,
    created: Json,
    started: Json,
    status: Json,
    result: Json,
}

/// Drive the whole lane through the CLI: real argv for every one of the four operations, and
/// `continuum-cli`'s own `Connection` for the typed half of the two reads.
///
/// Both halves are `continuum-cli`. `check::result` is `connection.verification_result(..)`
/// followed by `render_result(..)`, and `task::status` is the same shape, so reading the typed
/// answer through `Connection` reads it through the very call each command funnels through —
/// and both are `@readonly`, so asking twice is a re-read and not a second campaign.
fn drive_cli(rig: &mut Rig, task: &BenchmarkTask) -> CliRun {
    let components =
        String::from_utf8(to_bytes(&rig.components(task.source)).expect("the components encode"))
            .expect("canonical JSON is UTF-8");

    let created_json = invoke(
        rig,
        &argv(
            rig,
            &["snapshot", "create", "--components", &components, "--seal"],
            Principal::BUILDER,
        ),
    );
    let snapshot = WorkspaceHandle::new(&token(&created_json, "snapshot")).expect("a `ws_` handle");

    let ceiling = task.ceiling.to_string();
    let started_json = invoke(
        rig,
        &argv(
            rig,
            &[
                "check",
                "start",
                snapshot.as_str(),
                "--target",
                task.target_id,
                "--target-kind",
                task.target_kind.as_wire(),
                "--portfolio",
                Portfolio::Interactive.as_wire(),
                "--states",
                &ceiling,
            ],
            Principal::RUNNER,
        ),
    );
    let handle = TaskHandle::new(&token(&started_json, "task")).expect("a `task_` handle");

    let status_json = invoke(
        rig,
        &argv(rig, &["task", "status", handle.as_str()], Principal::RUNNER),
    );
    let result_json = invoke(
        rig,
        &argv(
            rig,
            &["check", "result", handle.as_str()],
            Principal::RUNNER,
        ),
    );

    // The typed half of the same session. `workspace.create` and `verification.start` are
    // `@mutation`s and are *not* re-sent: their typed answers are recovered by replay, which
    // `rule idempotency.replay` defines as the recorded first outcome returned verbatim.
    let mut connection = Connection::new(
        rig.version(),
        Principal::RUNNER.actor_id(),
        Principal::RUNNER.capability_handle(),
    );
    let status = connect(rig, &mut connection, CliCall::Status(handle.clone()));
    let result = connect(
        rig,
        &mut connection,
        CliCall::Result(VerificationResultRequest {
            task: handle.clone(),
        }),
    );

    // The two mutations' typed answers, replayed through the CLI's own connection under the
    // keys `continuum_cli::wire::Connection` generated for them. A replay is the protocol's
    // own way of asking "what did that call answer?" without running it twice.
    let created = replay_created(rig, task, &snapshot);
    let started = replay_started(rig, task, &snapshot, &handle);

    CliRun {
        produced: produced(
            rig,
            Script {
                snapshot,
                task: handle,
                created,
                started,
                status,
                result,
            },
        ),
        created: created_json,
        started: started_json,
        status: status_json,
        result: result_json,
    }
}

/// Replay the CLI's `workspace.create` through the CLI's own connection, under the key the CLI
/// generated for it (`cli-idem-{operation}-{counter}`, and every CLI invocation speaks exactly
/// one mutation as call one — see `Connection::with_idempotency_key`, bn-jmx97).
fn replay_created(rig: &mut Rig, task: &BenchmarkTask, snapshot: &WorkspaceHandle) -> Answered {
    let request = create_request(rig, task);
    let mut connection = Connection::new(
        rig.version(),
        Principal::BUILDER.actor_id(),
        Principal::BUILDER.capability_handle(),
    )
    .with_idempotency_key(Some("cli-idem-workspace.create-000001".to_owned()));
    let outcome = {
        let (server, pair, _) = rig.boundary();
        let mut link = CliLink::new(server, pair);
        connection
            .workspace_create(&mut link, &request)
            .expect("the call reaches a real frame")
    };
    let CliOutcome::Admitted(admitted) = outcome else {
        panic!("the replay of an admitted create was refused");
    };
    let Payload::WorkspaceCreate(response) = &admitted.payload else {
        panic!("a create answers a create payload");
    };
    assert_eq!(
        response.snapshot, *snapshot,
        "a replay returns the recorded first outcome (`rule idempotency.replay`)"
    );
    Answered {
        status: admitted.status,
        payload: admitted.payload,
        verdict: admitted.verdict.map_or(Optional::Absent, Optional::Present),
        assurance: admitted
            .assurance
            .map_or(Optional::Absent, Optional::Present),
        artifacts: admitted.artifacts,
        cost: admitted.cost,
        omissions: admitted.omissions,
    }
}

/// Replay the CLI's `verification.start`, as [`replay_created`] does for the create.
fn replay_started(
    rig: &mut Rig,
    task: &BenchmarkTask,
    snapshot: &WorkspaceHandle,
    handle: &TaskHandle,
) -> Answered {
    let mut connection = Connection::new(
        rig.version(),
        Principal::RUNNER.actor_id(),
        Principal::RUNNER.capability_handle(),
    )
    .with_idempotency_key(Some("cli-idem-verification.start-000001".to_owned()));
    let request = VerificationStartRequest {
        target: target(task),
        portfolio: Portfolio::Interactive,
        context_policy: Optional::Absent,
        priority_class: Optional::Absent,
    };
    let outcome = {
        let (server, pair, _) = rig.boundary();
        let mut link = CliLink::new(server, pair);
        connection
            .verification_start(&mut link, snapshot, &request, budget(task))
            .expect("the call reaches a real frame")
    };
    let CliOutcome::Admitted(admitted) = outcome else {
        panic!("the replay of an admitted start was refused");
    };
    let answered = Answered {
        status: admitted.status,
        payload: admitted.payload,
        verdict: admitted.verdict.map_or(Optional::Absent, Optional::Present),
        assurance: admitted
            .assurance
            .map_or(Optional::Absent, Optional::Present),
        artifacts: admitted.artifacts,
        cost: admitted.cost,
        omissions: admitted.omissions,
    };
    assert_eq!(
        started_task(&answered),
        *handle,
        "a replay returns the recorded first outcome (`rule idempotency.replay`)"
    );
    answered
}

// --- surface 3: the agent client -----------------------------------------------------------------

/// One `AgentClient` answer, folded into the shape every surface reports.
fn admitted(answer: &continuum_mcp::Answer, what: &str) -> Answered {
    let admitted = answer
        .success()
        .unwrap_or_else(|| panic!("the agent client was refused a {what} the others were given"));
    Answered {
        status: admitted.status,
        payload: admitted.payload.clone(),
        verdict: admitted
            .verdict
            .clone()
            .map_or(Optional::Absent, Optional::Present),
        assurance: admitted
            .assurance
            .clone()
            .map_or(Optional::Absent, Optional::Present),
        artifacts: admitted.artifacts.clone(),
        cost: admitted.cost.clone(),
        omissions: admitted.omissions.clone(),
    }
}

/// Drive the whole lane through `continuum-mcp`'s named typed operations.
fn drive_agent(rig: &mut Rig, task: &BenchmarkTask, budget: Budget) -> Produced {
    let mut client = AgentClient::new(
        rig.version(),
        Principal::BUILDER.actor_id(),
        Principal::BUILDER.capability_handle(),
    );
    let mut context = AgentContext::EMPTY;
    {
        let hello = continuum_benchmark::rig::hello();
        let (server, pair, welcome) = rig.boundary();
        let mut link = AgentLink::new(server, pair, welcome);
        client
            .open(&mut link, &hello)
            .expect("the connection opens");
    }

    let components = rig.components(task.source);
    let created = {
        let (server, pair, welcome) = rig.boundary();
        let mut link = AgentLink::new(server, pair, welcome);
        let answer = client
            .workspace_create(&mut link, &context, components, true)
            .expect("the call is made");
        let answered = admitted(&answer, "create");
        context.observe(
            "workspace.create",
            answer.success().expect("admitted just above"),
        );
        answered
    };
    let Payload::WorkspaceCreate(response) = &created.payload else {
        panic!("a create answers a create payload");
    };
    let snapshot = response.snapshot.clone();

    client.speak_as(
        Principal::RUNNER.actor_id(),
        Principal::RUNNER.capability_handle(),
    );
    let started = {
        let aim = target(task);
        let (server, pair, welcome) = rig.boundary();
        let mut link = AgentLink::new(server, pair, welcome);
        let answer = client
            .verification_start(&mut link, &context, &snapshot, aim, budget)
            .expect("the call is made");
        let answered = admitted(&answer, "start");
        context.observe(
            "verification.start",
            answer.success().expect("admitted just above"),
        );
        answered
    };
    let handle = started_task(&started);

    let status = {
        let (server, pair, welcome) = rig.boundary();
        let mut link = AgentLink::new(server, pair, welcome);
        let answer = client
            .task_status(&mut link, &context, &handle)
            .expect("the call is made");
        let answered = admitted(&answer, "status");
        context.observe(
            "task.status",
            answer.success().expect("admitted just above"),
        );
        answered
    };

    let result = {
        let (server, pair, welcome) = rig.boundary();
        let mut link = AgentLink::new(server, pair, welcome);
        let answer = client
            .verification_result(&mut link, &context, &handle)
            .expect("the call is made");
        admitted(&answer, "result")
    };

    produced(
        rig,
        Script {
            snapshot,
            task: handle,
            created,
            started,
            status,
            result,
        },
    )
}

// --- the triple ------------------------------------------------------------------------------------

/// Drive one fixture through all three surfaces, each against its own fresh deployment.
///
/// Three deployments rather than one, deliberately. Sharing a daemon would make the second and
/// third surfaces resolve to the first's campaign by content identity — a true fact, and a
/// *weaker* one than the exit asks for. Independently provisioned deployments driven by three
/// different surfaces converging on the same bytes is the claim.
fn triple(task: &BenchmarkTask) -> ([Produced; 3], CliRun) {
    let native = {
        let mut rig = Rig::fresh();
        drive_native(&mut rig, task, budget(task))
    };
    let cli = {
        let mut rig = Rig::fresh();
        drive_cli(&mut rig, task)
    };
    let agent = {
        let mut rig = Rig::fresh();
        drive_agent(&mut rig, task, budget(task))
    };
    ([native, cli.produced.clone(), agent], cli)
}

/// Assert the three records are identical, naming the two surfaces that disagreed.
fn assert_identical(task: &BenchmarkTask, runs: &[Produced; 3]) {
    for (index, run) in runs.iter().enumerate().skip(1) {
        let (left, right) = (Surface::ALL[0].token(), Surface::ALL[index].token());
        let id = task.id;
        assert_eq!(
            runs[0].snapshot, run.snapshot,
            "{id}: {left} and {right} named different snapshots"
        );
        assert_eq!(
            runs[0].task, run.task,
            "{id}: {left} and {right} named different campaigns"
        );
        assert_eq!(
            runs[0].published, run.published,
            "{id}: {left} and {right} published different artifacts — content address or bytes"
        );
        assert_eq!(
            runs[0].receipts, run.receipts,
            "{id}: {left} and {right} left different publication receipts"
        );
        assert_eq!(
            runs[0].attribution, run.attribution,
            "{id}: {left} and {right} stored different bytes per class"
        );
        for (expected, actual) in runs[0].answers.iter().zip(&run.answers) {
            assert_eq!(
                expected.0, actual.0,
                "{id}: the two scripts diverged in shape"
            );
            assert_eq!(
                expected.1,
                actual.1,
                "{id}: {left} and {right} answered {} with different canonical bytes\n{left}: \
                 {}\n{right}: {}",
                expected.0,
                String::from_utf8_lossy(&expected.1),
                String::from_utf8_lossy(&actual.1),
            );
        }
        assert_eq!(
            runs[0].answers.len(),
            run.answers.len(),
            "{id}: {left} and {right} ran scripts of different lengths"
        );
        assert_eq!(
            runs[0].states, run.states,
            "{id}: {left} and {right} reported different reachable-state counts"
        );
    }
}

/// Assert one run reproduces the dossier's frozen answer, so an identity across three surfaces
/// cannot be an identity across three wrong answers.
fn assert_frozen(task: &BenchmarkTask, run: &Produced) {
    assert_eq!(
        run.verdict, task.expected.verdict,
        "{}: the folded verdict is the dossier's",
        task.id
    );
    assert_eq!(
        run.states,
        Optional::Present(task.expected.states),
        "{}: the reachable-state count is the dossier's, read off the wire",
        task.id
    );
    assert!(
        !run.published.is_empty(),
        "{}: a campaign that published nothing would make every equality above vacuous",
        task.id
    );
    assert!(
        !run.receipts.is_empty(),
        "{}: a store that issued no receipt has no receipt to compare",
        task.id
    );
    assert!(
        !run.start_refs.is_empty(),
        "{}: a campaign that named no artifact ref has no content address to compare",
        task.id
    );
}
// --- the evidence ------------------------------------------------------------------------------------

/// **The exit sentence this file answers is still the plan's.**
///
/// A reworded exit fails here rather than leaving the file below answering a sentence that
/// moved, and §21.1's permanence rule is cited at the same time because it is what makes this
/// a regression rather than a one-time report.
#[test]
fn the_exit_sentence_this_file_answers_is_still_the_plans() {
    assert!(
        PLAN_MD.contains(EXIT_SENTENCE),
        "plan §21's Phase A exit sentence is no longer the one this file answers"
    );
    assert!(
        PLAN_MD.contains(REGRESSION_RULE),
        "plan §21.1's permanent-regression rule is what makes this file a test"
    );
}

/// **Identical explicit inputs, checked rather than assumed (INV-005).**
///
/// The whole check is meaningless if the three surfaces are not asked the same question, and
/// two of the three build their budget through their own crate's declaration. This asserts the
/// two declarations are one value, that the request shape the three surfaces build is the same
/// shape, and that two fresh deployments are the same deployment.
#[test]
fn the_three_surfaces_declare_one_budget_and_one_request_shape() {
    for states in [16_u64, 64, 573, 1_024] {
        assert_eq!(
            continuum_cli::task::states_budget(states),
            continuum_mcp::client::states_budget(states),
            "the CLI and the agent client must mean one thing by a states budget"
        );
    }
    // The create request all three send: `overlay` absent, `seal` present-and-true. The CLI
    // builds it in `snapshot::CreateArgs::request` and the agent client in
    // `AgentClient::workspace_create`; this pins the shape the native arm restates.
    let rig = Rig::fresh();
    let request = create_request(&rig, &DIE_HARD_ALL);
    assert_eq!(request.overlay, Optional::Absent);
    assert_eq!(request.seal, Optional::Present(true));
    assert_eq!(request.components, rig.components(DIE_HARD_ALL.source));
    // And the deployment each arm is given is the same deployment.
    let second = Rig::fresh();
    assert_eq!(rig.intent(), second.intent());
    assert_eq!(rig.version(), second.version());
    assert_eq!(rig.welcome_frame(), second.welcome_frame());
    assert_eq!(
        rig.components_reference(DIE_HARD_ALL.source),
        second.components_reference(DIE_HARD_ALL.source)
    );
    let stored = read_store(rig.daemon());
    assert!(
        stored.published.is_empty() && stored.receipts.is_empty(),
        "a fresh deployment has published nothing, so every artifact below was produced by a \
         surface"
    );
}

/// **Die Hard: byte-identical artifacts and receipts through all three surfaces.**
#[test]
fn die_hard_publishes_byte_identical_artifacts_through_all_three_surfaces() {
    let (runs, _) = triple(&DIE_HARD_ALL);
    assert_identical(&DIE_HARD_ALL, &runs);
    for run in &runs {
        assert_frozen(&DIE_HARD_ALL, run);
    }
}

/// **Dining Philosophers: byte-identical artifacts and receipts through all three surfaces.**
///
/// The second of the two models the exit names, and the one whose absence would have made this
/// a half-answer. It is drivable because `continuum_benchmark::philosophers` declares it as a
/// `Model` in the engine's own vocabulary and `Rig::fresh` registers it beside Die Hard.
#[test]
fn dining_philosophers_publishes_byte_identical_artifacts_through_all_three_surfaces() {
    let (runs, _) = triple(&PHILOSOPHERS_ALL);
    assert_identical(&PHILOSOPHERS_ALL, &runs);
    for run in &runs {
        assert_frozen(&PHILOSOPHERS_ALL, run);
    }
}

/// **The typed answer is byte-identical across all three surfaces, on every operation.**
///
/// Separate from the store comparison because it is a different claim: the store says the
/// three campaigns *produced* one artifact set, and this says the three surfaces *reported*
/// one answer, down to canonical bytes — the payload, the artifact refs, the verdict, the
/// assurance envelope, the cost, and the INV-007 manifest, on each of the four operations.
#[test]
fn the_typed_verification_result_is_byte_identical_across_all_three_surfaces() {
    for task in [&DIE_HARD_ALL, &PHILOSOPHERS_ALL] {
        let (runs, _) = triple(task);
        assert_eq!(
            runs[0].answers.len(),
            4,
            "the script is four operations long"
        );
        for (operation, bytes) in &runs[0].answers {
            assert!(
                !bytes.is_empty() && *bytes != b"{}".to_vec(),
                "{}: an empty {operation} encoding would make the equality below vacuous",
                task.id
            );
        }
        for (index, run) in runs.iter().enumerate().skip(1) {
            assert_eq!(
                runs[0].answers,
                run.answers,
                "{}: {} and {} disagree on the canonical answer bytes",
                task.id,
                Surface::ALL[0].token(),
                Surface::ALL[index].token()
            );
        }
    }
}

/// **The CLI's rendering is a projection of the same artifact, and names no other.**
///
/// The CLI is the one surface whose answer to a human is text. This decodes that text and
/// asserts every content-addressed token in it is one the other two surfaces produced — so the
/// rendering neither invents an identity nor loses one. INV-003's "human text may accompany a
/// machine result, never define it", read as a test.
#[test]
fn the_cli_rendering_projects_the_same_content_addresses_and_invents_none() {
    for task in [&DIE_HARD_ALL, &PHILOSOPHERS_ALL] {
        let (runs, cli) = triple(task);
        let native = &runs[0];
        let id = task.id;

        assert_eq!(
            token(&cli.created, "snapshot"),
            native.snapshot.as_str(),
            "{id}: the CLI printed a snapshot the other surfaces did not name"
        );
        assert_eq!(
            token(&cli.started, "task"),
            native.task.as_str(),
            "{id}: the CLI printed a campaign the other surfaces did not name"
        );
        assert_eq!(
            token(&cli.status, "task"),
            native.task.as_str(),
            "{id}: the rendered task record is about the same campaign"
        );
        assert_eq!(
            token(field(&cli.result, "result"), "task"),
            native.task.as_str(),
            "{id}: the rendered result body names the same campaign"
        );

        // Every artifact ref the `check start` rendering carries is the typed ref the other two
        // surfaces were answered with, compared as a whole canonical document rather than as a
        // scraped substring.
        let rendered = field(&cli.started, "artifacts")
            .as_array()
            .expect("the artifact list is an array");
        let typed: Vec<Json> = native.start_refs.iter().map(as_json).collect();
        assert_eq!(
            rendered.len(),
            typed.len(),
            "{id}: the CLI rendered a different number of artifact refs"
        );
        for (rendered, typed) in rendered.iter().zip(&typed) {
            assert_eq!(
                token(rendered, "handle"),
                token(typed, "handle"),
                "{id}: the CLI rendered an artifact handle the wire did not carry"
            );
            assert_eq!(
                token(rendered, "commitment"),
                token(typed, "commitment"),
                "{id}: the CLI rendered a commitment the wire did not carry"
            );
            assert_eq!(
                token(rendered, "kind"),
                token(typed, "kind"),
                "{id}: the CLI rendered an artifact class the wire did not carry"
            );
            assert!(
                native
                    .published
                    .iter()
                    .any(|(address, _)| address.to_string() == token(rendered, "commitment")),
                "{id}: the CLI rendered a commitment no deployment published"
            );
        }

        // And the verdict token the human reads is the verdict the wire carried.
        assert_eq!(
            token(field(&cli.result, "verdict"), "verdict"),
            native.verdict.as_wire(),
            "{id}: the rendered verdict token is the wire's"
        );
    }
}

/// **Anti-vacuity: every published address is the Blake3 identity of its own bytes.**
///
/// "The hashes matched" is only evidence if a hash names the bytes beside it. This recomputes
/// every content address in every deployment from the content it was read with, through the
/// same `ContentIdentifier` the daemon publishes under — so an equality between two stores of
/// mislabelled or empty content cannot pass.
#[test]
fn every_published_address_is_the_blake3_identity_of_its_own_bytes() {
    for task in [&DIE_HARD_ALL, &PHILOSOPHERS_ALL] {
        let (runs, _) = triple(task);
        for (index, run) in runs.iter().enumerate() {
            assert!(
                !run.published.is_empty(),
                "{}: {} published nothing",
                task.id,
                Surface::ALL[index].token()
            );
            for (address, bytes) in &run.published {
                assert!(
                    !bytes.is_empty(),
                    "{}: {} published an empty artifact",
                    task.id,
                    Surface::ALL[index].token()
                );
                let recomputed =
                    ContentIdentifier::identify(&Blake3Identity, address.class(), bytes)
                        .expect("blake3 names every input");
                assert_eq!(
                    recomputed,
                    *address,
                    "{}: {} holds an artifact whose address does not name its bytes",
                    task.id,
                    Surface::ALL[index].token()
                );
            }
            // And the receipt ledger names exactly the published identities: a receipt for an
            // unpublished identity would be a forged one, and a publication with no receipt
            // would be a lost one (G0-DX-13).
            for receipt in &run.receipts {
                assert!(
                    run.published
                        .iter()
                        .any(|(address, _)| address == receipt.handle()),
                    "{}: {} holds a receipt for an identity its index does not name",
                    task.id,
                    Surface::ALL[index].token()
                );
            }
        }
    }
}

/// **Anti-vacuity: one changed input changes the artifacts.**
///
/// The equalities above would be worthless if the artifacts were a constant. This drives the
/// native arm twice with one input changed — the declared state ceiling, which
/// `daemon::verification::start_handle` folds into the campaign identity — and asserts the
/// produced artifacts differ. It is also the reason the three arms are held to one budget: a
/// triple-surface check that let each surface pick its own would be comparing three campaigns,
/// not three surfaces.
#[test]
fn one_changed_input_changes_the_artifacts_so_the_equality_above_is_a_constraint() {
    let task = &DIE_HARD_ALL;
    let baseline = {
        let mut rig = Rig::fresh();
        drive_native(&mut rig, task, budget(task))
    };
    let altered = {
        let mut rig = Rig::fresh();
        drive_native(
            &mut rig,
            task,
            continuum_cli::task::states_budget(task.ceiling + 1),
        )
    };
    assert_eq!(
        baseline.snapshot, altered.snapshot,
        "the snapshot is a function of the components, which did not change"
    );
    assert_ne!(
        baseline.task, altered.task,
        "the campaign identity folds in the declared budget"
    );
    assert_ne!(
        baseline.published, altered.published,
        "a different campaign publishes a different record"
    );
    assert_ne!(
        baseline.answers, altered.answers,
        "and the answers differ, so the byte equality above is a constraint"
    );
    assert_eq!(
        baseline.verdict, altered.verdict,
        "and the verdict is unchanged: the input that moved was the ceiling, not the question"
    );
    assert_eq!(
        baseline.states, altered.states,
        "as is the reachable set — the ceiling bounds the search, it does not change the model"
    );
}

/// **The reading of "agent client" is the one its own crate claims.**
///
/// Recorded as a test rather than as prose because the reading is a decision this bone made,
/// and a later reader is owed the grounds mechanically. The texts asserted here are the ones
/// that decide it.
#[test]
fn the_agent_client_reading_is_the_one_its_own_crate_claims() {
    assert!(
        MCP_LIB.contains("crate list names exactly one agent-facing client surface"),
        "`continuum-mcp` no longer claims to be the agent client the exit sentence names"
    );
    assert!(
        MCP_LIB.contains("agent client** with identical artifacts"),
        "`continuum-mcp` no longer cites the exit sentence this file answers"
    );
    assert!(
        PLAN_MD.contains("MCP-only surface"),
        "plan §24.5's DX-10 fallback no longer names the surface the ablation grades"
    );
    // And the two surfaces that could have been mistaken for it disclaim the role in their own
    // words: the benchmark's baseline arm is a text projection, not `continuum-cli`.
    assert!(
        BENCHMARK_SHELL.contains("there is no `continuum` binary to shell out to"),
        "`continuum_benchmark::shell` no longer says it is a simulacrum rather than the CLI"
    );
    assert!(
        CLI_MANIFEST.contains("continuumd = { path = \"../continuumd\" }"),
        "the CLI's single production edge is continuumd (bn-jmx97); a second edge would make \
         'the CLI is a projection' a claim about a different graph"
    );
}

/// **The absences this check runs around are still absent.**
///
/// Each is a typed absence with a tripwire: the assertion fails when the absence is filled, so
/// the day a producer lands, this file is edited rather than quietly continuing to describe a
/// world that moved. This is the house pattern, not a skip.
#[test]
fn the_absences_this_check_runs_around_are_still_absent() {
    // 1. There is no Dining Philosophers Intent Contract fixture. Both ports run under the Die
    //    Hard contract, deliberately, so the comparison holds intent fixed.
    assert!(
        CORPUS.contains(
            "include_str!(\"../../continuum-intent/tests/fixtures/die-hard-contract.json\")"
        ),
        "a Dining Philosophers contract may have landed; if so this file should run TV-007 \
         under its own intent"
    );
    assert!(
        !std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../continuum-intent/tests/fixtures/dining-philosophers-contract.json"
        ))
        .exists(),
        "a Dining Philosophers contract fixture landed; the absence above is stale"
    );

    // 2. No certificate is emitted for Dining Philosophers. Emission exists and is exercised for
    //    Die Hard only, in `continuum-mcp`'s own evidence.
    assert!(
        MCP_TYPED_SURFACE.contains("emit_finite_closure"),
        "Die Hard's certificate lane moved; the philosophers absence below is no longer scoped \
         against it"
    );
    assert!(
        !MCP_TYPED_SURFACE.contains("philosophers"),
        "a philosophers certificate lane landed; this check should compare certificates too"
    );

    // 3. No crashpack producer exists, so neither the depth-6 solution nor the depth-10 deadlock
    //    has an artifact to compare. The CLI's own pinned golden says so.
    assert!(
        CLI_CHECK_GOLDEN.contains("\"crashpack\":null"),
        "a crashpack producer landed; the witness is now an artifact this check must compare"
    );
    assert!(
        CLI_CHECK_GOLDEN.contains("\"reason\":\"no-certificate-emitted\""),
        "the daemon now emits a certificate; its identity belongs in the comparison above"
    );

    // 4. Transitions and witness depth have no wire field, so the frozen 96/2,365 and the two
    //    depths are corroborated one layer down and are deliberately not part of the
    //    cross-surface artifact comparison (RFC 0026 F16).
    let golden = Json::parse(CLI_CHECK_GOLDEN.trim_end().as_bytes())
        .expect("the pinned golden is one canonical JSON document");
    let dimensions = field(&golden, "cost")
        .as_object()
        .expect("`Cost` is an object");
    for task in [&DIE_HARD_ALL, &PHILOSOPHERS_ALL] {
        for dimension in dimensions.keys() {
            assert!(
                !dimension.contains("transition") && !dimension.contains("witness"),
                "{}: `Cost` grew a {dimension:?} dimension; the frozen {} transitions and depth \
                 {} can now cross the wire and belong in the comparison",
                task.id,
                task.expected.transitions,
                task.expected.witness_depth
            );
        }
    }

    // 5. `continuum-cli`'s default protocol version still trails the deployment's, which is why
    //    every CLI invocation above passes `--protocol-version` explicitly.
    let deployed = Rig::fresh();
    let (major, minor) = continuum_cli::cli::DEFAULT_PROTOCOL_VERSION;
    assert!(
        (major, minor) != (deployed.version().major(), deployed.version().minor()),
        "the CLI's DEFAULT_PROTOCOL_VERSION caught up with the deployment; the explicit \
         `--protocol-version` flag in `argv` is now redundant and should be dropped"
    );
}
