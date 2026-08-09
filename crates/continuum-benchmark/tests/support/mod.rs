//! The native arm: the typed client, wired as a [`Surface`].
//!
//! # Why this lives in `tests/` and not in `src/`
//!
//! `continuum-mcp` is an adapter, and plan §20's "adapters do not own semantic state" is
//! enforced mechanically as "adapters are sinks in the workspace graph"
//! (`tools/check_crate_boundaries.py`, RULE adapters-are-sinks): no crate may take a normal
//! dependency on one. A `tests/*.rs` file is its own crate and links the client as a
//! **dev-dependency**, which that checker reports rather than enforces, by its own documented
//! design ("a differential test may legitimately link an engine it would never ship against").
//!
//! What that costs is exactly this file, and it is worth being precise about how little it
//! is. Everything that *scores* a run — attempt counting, admission classification, byte
//! accounting, view folding, completion, the transcript line — is in `continuum_benchmark::run`
//! and is applied identically to both arms. This file decides two things and no others:
//! which client method a [`Step`] becomes, and what the typed answer let the agent read.
//! `continuum_benchmark::shell` decides the same two things for the other arm.
//!
//! # The reading rule, and its symmetry with the shell arm's
//!
//! [`read`] fills a [`Reading`] from typed fields. Two rules keep it symmetric with the
//! baseline's scrape rather than quietly generous:
//!
//! - **`continuation_known` is true on every admitted answer.** RFC 0026 makes the envelope's
//!   `continuation` "present when `status = task_suspended`, and on a resumable failure", so
//!   its absence on any other admitted answer is a *statement* that nothing is parked — not
//!   silence. The shell projection prints `continuation: none` for the same reason, so both
//!   arms decide the question on every answer.
//! - **`ceiling_enforced` is read from the omission manifest**, which the typed answer carries
//!   inline and the projection summarizes. This is the one datum the two arms pay differently
//!   for, and `continuum_benchmark::shell`'s "one lossy rule" says why that is the honest
//!   place to pay it.
//!
//! [`Step`]: continuum_benchmark::policy::Step
//! [`Surface`]: continuum_benchmark::surface::Surface

// A shared `tests/support` module is compiled into every test binary that declares it, and
// no single binary uses every helper. The alternative is five copies of this file.
#![allow(dead_code)]

use continuum_benchmark::families::{self, MistakeFamily};
use continuum_benchmark::policy::{Call, Policy, PolicyKind, SCHEDULES, Step};
use continuum_benchmark::rig::{Principal, Rig};
use continuum_benchmark::run::{self, ArmRun};
use continuum_benchmark::surface::{Arm, Observation, Reading, Surface, SurfaceError};
use continuum_benchmark::task::{BenchmarkTask, SUBSET};
use continuum_mcp::answer::Answer;
use continuum_mcp::{AgentClient, AgentContext, LocalLink};
use continuumd::daemon::family::Payload;
use continuumd::protocol::envelope::Verdict;
use continuumd::protocol::shared::{FileOverlay, Target};
use continuumd::protocol::spec::Optional;

/// The typed arm.
#[derive(Debug)]
pub struct NativeSurface {
    client: AgentClient,
    context: AgentContext,
    opened: bool,
}

impl Default for NativeSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeSurface {
    /// A client that has not opened a connection yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: AgentClient::new(
                continuum_benchmark::rig::version(),
                Principal::BUILDER.actor_id(),
                Principal::BUILDER.capability_handle(),
            ),
            context: AgentContext::EMPTY,
            opened: false,
        }
    }

    /// The grammar state this arm currently holds.
    #[must_use]
    pub const fn context(&self) -> AgentContext {
        self.context
    }

    /// The client's byte ledger.
    #[must_use]
    pub const fn ledger(&self) -> continuum_mcp::ByteLedger {
        self.client.ledger()
    }

    fn fail(operation: &'static str, detail: impl core::fmt::Display) -> SurfaceError {
        SurfaceError {
            arm: Arm::Native,
            operation,
            detail: detail.to_string(),
        }
    }
}

impl Surface for NativeSurface {
    fn arm(&self) -> Arm {
        Arm::Native
    }

    fn handshake_bytes(&self) -> u64 {
        self.client.ledger().handshake.total()
    }

    fn perform(
        &mut self,
        rig: &mut Rig,
        task: &BenchmarkTask,
        step: &Step,
    ) -> Result<Observation, SurfaceError> {
        let operation = step.call.operation();
        // A mistake in how a call is *written* has no channel here. `AgentClient` reaches
        // every one of its eight operations through a method whose parameters are typed
        // values: an argument's name is a struct field the compiler resolves, and a missing
        // parameter is a compile error rather than a call. There is nothing to compose and
        // nothing to refuse, so nothing is attempted — see
        // `continuum_benchmark::surface::Observation::unrepresentable` for why that is not
        // the same as this arm's own zero-cost local refusal.
        if let Some(mistake) = step.mistake.filter(|mistake| mistake.written()) {
            return Ok(Observation {
                admitted: false,
                code: None,
                local_refusal: false,
                unrepresentable: true,
                bytes: 0,
                reading: Reading::default(),
                line: families::mistake_line(step, mistake, false, 0),
            });
        }
        let rig_reference = rig.components_reference(task.source);
        let rig_intent = rig.intent().clone();
        let hello = continuum_benchmark::rig::hello();
        let target = Target {
            kind: task.target_kind,
            id: task.target_id.to_owned(),
        };
        self.client.speak_as(
            step.principal.actor_id(),
            step.principal.capability_handle(),
        );

        let (server, pair, welcome) = rig.boundary();
        let mut link = LocalLink::new(server, pair, welcome);
        if !self.opened {
            self.client
                .open(&mut link, &hello)
                .map_err(|error| Self::fail(operation, error))?;
            self.opened = true;
        }

        let context = self.context;
        let answer = match &step.call {
            // Protocol 3.6: the typed arm names the port's component set by its content
            // identity, which is what the shell arm's `--port TV-009` has always done. The
            // *act* is unchanged — `Call::operation()` still reports `workspace.create`,
            // both arms attempt the identical sequence, and the same snapshot comes back —
            // and what changed is the argument. `tests/pr10_c4b_port_by_reference.rs`
            // holds the delta, and holds the shell arm's numbers still.
            Call::CreateWorkspace { seal } => self.client.workspace_create_by_reference(
                &mut link,
                &context,
                rig_reference,
                continuum_benchmark::rig::snapshot_epochs(),
                rig_intent,
                *seal,
            ),
            Call::ForkModule { base } => self.client.workspace_fork(
                &mut link,
                &context,
                base,
                vec![FileOverlay {
                    path: task.source.module_path().to_owned(),
                    content: continuum_benchmark::policy::EDITED_MODULE.to_vec(),
                }],
            ),
            Call::RestoreModule { base } => self.client.workspace_fork(
                &mut link,
                &context,
                base,
                vec![FileOverlay {
                    path: task.source.module_path().to_owned(),
                    content: task.source.module().as_bytes().to_vec(),
                }],
            ),
            Call::SealWorkspace { snapshot } => {
                self.client.workspace_seal(&mut link, &context, snapshot)
            }
            Call::StartVerification { snapshot, states } => self.client.verification_start(
                &mut link,
                &context,
                snapshot,
                target,
                continuum_benchmark::declared_budget(*states),
            ),
            Call::PollTask { task: handle } => self.client.task_status(&mut link, &context, handle),
            Call::FetchResult { task: handle } => {
                self.client.verification_result(&mut link, &context, handle)
            }
            Call::Resume {
                continuation,
                states,
            } => self.client.task_resume(
                &mut link,
                &context,
                continuation,
                continuum_benchmark::declared_budget(*states),
            ),
            Call::Cancel { task: handle } => self.client.task_cancel(&mut link, &context, handle),
        }
        .map_err(|error| Self::fail(operation, error))?;

        if let Some(admitted) = answer.success() {
            self.context.observe(operation, admitted);
        }
        let bytes = answer.bytes.total();
        let admitted = answer.admitted();
        let code = answer.error_code();
        Ok(Observation {
            admitted,
            code,
            local_refusal: answer.unmet().is_some(),
            unrepresentable: false,
            bytes,
            reading: read(operation, &answer),
            line: run::line(step, admitted, code, bytes),
        })
    }
}

/// What the typed answer let the agent read.
#[must_use]
pub fn read(operation: &str, answer: &Answer) -> Reading {
    let mut reading = Reading::default();
    let Some(admitted) = answer.success() else {
        return reading;
    };
    match &admitted.payload {
        Payload::WorkspaceCreate(response) => {
            reading.snapshot = Some(response.snapshot.clone());
            reading.sealed = Some(response.sealed);
        }
        // The 3.6 spelling of the same act, and the same two facts. The bodies are
        // member-for-member equal by construction (`rule snapshot.by_reference`), so this
        // arm reads exactly what the inline arm read.
        Payload::WorkspaceCreateByReference(response) => {
            reading.snapshot = Some(response.snapshot.clone());
            reading.sealed = Some(response.sealed);
        }
        Payload::WorkspaceFork(response) => {
            reading.snapshot = Some(response.snapshot.clone());
            reading.sealed = Some(false);
        }
        Payload::WorkspaceSeal(response) => {
            reading.snapshot = Some(response.snapshot.clone());
            reading.sealed = Some(true);
        }
        Payload::TaskStatus(record) => {
            reading.task = Some(record.task.clone());
            reading.status = Some(record.status);
            reading.states = record.cost.states.value().copied();
            reading.continuation = record.continuation.value().cloned();
        }
        Payload::TaskResume(response) => {
            reading.task = Some(response.task.clone());
            reading.status = Some(response.status);
        }
        Payload::TaskCancel(response) => {
            reading.task = Some(response.task.clone());
            reading.status = Some(response.status);
            if let continuumd::protocol::spec::Nullable::Value(handle) = &response.continuation {
                reading.continuation = Some(handle.clone());
            }
        }
        Payload::VerificationResult(result) | Payload::VerificationAwait(result) => {
            reading.task = Some(result.task.clone());
            if let Some(handle) = result.continuation.value() {
                reading.continuation = Some(handle.clone());
            }
        }
        _ => {}
    }
    if reading.task.is_none() {
        reading.task = admitted.task.clone();
    }
    if reading.continuation.is_none() {
        reading.continuation = admitted.continuation.clone();
    }
    // See the module documentation: an admitted answer decides the continuation question,
    // whichever way, on both arms.
    reading.continuation_known = true;
    if let Some(Verdict::Semantic(value)) = &admitted.verdict {
        reading.verdict = Some(value.verdict);
    }
    reading.omissions = Some(admitted.omissions.len());
    if matches!(operation, "verification.start" | "task.resume") {
        reading.ceiling_enforced = Some(
            !admitted
                .omissions
                .iter()
                .any(|omission| omission.subject == "budget.states"),
        );
    }
    reading
}

/// Drive the native arm over the whole matrix, one fresh rig per cell.
///
/// # Panics
///
/// When an arm cannot attempt an operation at all, which means this crate is wrong rather
/// than that an arm lost.
#[must_use]
pub fn native_sweep() -> Vec<ArmRun> {
    let mut runs = Vec::new();
    for task in SUBSET {
        for schedule in SCHEDULES {
            for kind in PolicyKind::ALL {
                let mut rig = Rig::fresh();
                let mut surface = NativeSurface::new();
                runs.push(
                    run::drive(
                        &mut surface,
                        &mut rig,
                        task,
                        Policy::new(kind, schedule.seed),
                    )
                    .expect("the native arm attempts every operation"),
                );
            }
        }
    }
    runs
}

/// Drive the native arm over one fallible-policy family's whole matrix.
///
/// The mirror of `continuum_benchmark::families::shell_sweep`, and it has to live here for
/// the same plan §20 reason the rest of this file does. The cell enumeration is
/// `families::cells`, shared with the baseline, so neither arm decides which cell is which.
///
/// # Panics
///
/// As [`native_sweep`].
#[must_use]
pub fn native_family_sweep(family: MistakeFamily) -> Vec<ArmRun> {
    let mut runs = Vec::new();
    for (task, seed, kind, cell) in families::cells() {
        let mut rig = Rig::fresh();
        let mut surface = NativeSurface::new();
        runs.push(
            run::drive(
                &mut surface,
                &mut rig,
                task,
                families::policy_for(family, seed, kind, cell),
            )
            .expect("the native arm attempts every operation"),
        );
    }
    runs
}

/// Both arms over one family's matrix: `(native, shell)`.
///
/// # Panics
///
/// As [`native_sweep`].
#[must_use]
pub fn family_arms(family: MistakeFamily) -> (Vec<ArmRun>, Vec<ArmRun>) {
    (
        native_family_sweep(family),
        families::shell_sweep(family).expect("the shell arm attempts every operation"),
    )
}

/// Drive both arms over the whole matrix.
///
/// # Panics
///
/// As [`native_sweep`].
#[must_use]
pub fn both_arms() -> Vec<ArmRun> {
    let mut runs = native_sweep();
    runs.extend(run::sweep_shell().expect("the shell arm attempts every operation"));
    runs
}

/// Drive one cell on the native arm, returning the run and the surface that produced it.
///
/// # Panics
///
/// As [`native_sweep`].
#[must_use]
pub fn native_cell(task: &BenchmarkTask, kind: PolicyKind, seed: u16) -> (ArmRun, NativeSurface) {
    let mut rig = Rig::fresh();
    let mut surface = NativeSurface::new();
    let run = run::drive(&mut surface, &mut rig, task, Policy::new(kind, seed))
        .expect("the native arm attempts every operation");
    (run, surface)
}

/// Drive one cell on the shell arm.
///
/// # Panics
///
/// As [`native_sweep`].
#[must_use]
pub fn shell_cell(task: &BenchmarkTask, kind: PolicyKind, seed: u16) -> ArmRun {
    let mut rig = Rig::fresh();
    let mut surface = continuum_benchmark::shell::ShellSurface::new();
    run::drive(&mut surface, &mut rig, task, Policy::new(kind, seed))
        .expect("the shell arm attempts every operation")
}

/// The `Optional` value of a field, for a test that needs one.
#[must_use]
pub fn present<T: Clone>(value: &Optional<T>) -> Option<T> {
    value.value().cloned()
}
