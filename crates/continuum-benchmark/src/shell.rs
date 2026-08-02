//! The disciplined shell-scraping baseline.
//!
//! # What this is a baseline *of*
//!
//! PR 13's human CLI does not exist yet, so there is no `continuum` binary to shell out to.
//! Rather than build one — that is PR 13's deliverable and inventing it here would ship a
//! second, unratified command surface — this module is the **text-rendering adapter** the
//! goal bone's note allows: the same daemon, over the same wire, with its typed answers
//! projected into the kind of output a human CLI prints, and an agent half that recovers
//! facts from that text and nothing else.
//!
//! Two consequences follow, and both are stated rather than buried:
//!
//! - **The rendering is this crate's**, so it is this crate's job not to strawman it. What
//!   guards against that is stated in "the projection contract" below, and the contract is
//!   mechanical enough to check.
//! - **The baseline is not charged for the CLI's own protocol traffic.** A real `continuum`
//!   process would exchange frames with the daemon, and those bytes never reach the agent. So
//!   the shell arm's interface bytes are the **command line it wrote plus the output it
//!   read**, and the frames underneath are free. That is a deliberate handicap on the native
//!   arm, which pays for every byte of its envelope, and it is the honest boundary: the
//!   agent's interface to a CLI is stdin and stdout.
//!
//! # The projection contract
//!
//! [`render`] is total, deterministic, and derived. Specifically:
//!
//! 1. **No invented content.** Every line is a projection of a field of the
//!    [`ResultEnvelope`] or of the operation's typed payload. Nothing is padded, and no
//!    explanatory prose is added that the typed answer does not carry.
//! 2. **Wire tokens, not synonyms.** `status: task_suspended`, `verdict: refuted`,
//!    `error.code: capability-denied` — the CLI prints the protocol's own closed-vocabulary
//!    tokens, so the baseline's parse is exact rather than fuzzy. A CLI that printed "the
//!    task was paused" would be a strawman and this one does not.
//! 3. **Handles in full.** No truncation, no ellipsis, no `…`. A CLI that abbreviated
//!    handles would hand the baseline a failure this benchmark did not earn.
//! 4. **Every scalar is present or explicitly `none`.** Absence is written down, so
//!    "the answer says there is no continuation" and "the answer does not mention
//!    continuations" are as distinguishable in the text as they are on the wire. This is the
//!    single most generous rule in the contract and it is deliberate: without it the
//!    baseline would lose on an accounting artefact rather than on interface design.
//! 5. **List-valued fields are summarized, with an expansion command.** `omissions: 2
//!    (expand: …)`. This is what a human CLI does, and it is the one place the projection is
//!    genuinely lossy — see below.
//!
//! # The one lossy rule, and why it is the honest one
//!
//! Rule 5 is where the typed surface and the text surface actually differ. RFC 0026 makes
//! `omissions` a **required** field of every result envelope, so the typed arm receives the
//! whole INV-007 manifest inline, in the same answer, for no extra byte. A human CLI prints
//! a count and offers an expansion, because a terminal is not a place to print a structured
//! list nobody asked for.
//!
//! That difference is only worth measuring if the agent actually needs the manifest, so it
//! does: [`crate::policy::AgentView::ceiling_enforced`] asks whether the state ceiling the
//! agent declared was *metered*, which is exactly the question the manifest answers and
//! exactly the question `continuumd::daemon::verification` says the manifest exists to
//! answer ("silently ignoring a declared ceiling is exactly the failure that rule exists to
//! prevent"). The native arm reads it off the answer it already has. This arm issues the
//! expansion command and pays for it. Neither arm is charged for information the policy does
//! not use.
//!
//! # The four disciplines
//!
//! 1. **Anchored key lookup.** [`field`] matches a line whose prefix is exactly `key: `. It
//!    never matches positionally, never scans for a substring, and never guesses from
//!    ordering.
//! 2. **Closed-vocabulary parsing.** Tokens are parsed with the protocol's own
//!    [`ProtocolEnum::from_wire`], so an unrecognized token is a *miss* rather than a
//!    plausible-looking wrong answer.
//! 3. **Explicit expansion.** A summarized list is expanded by command, never assumed.
//! 4. **Bounded re-read.** When a required key is missing, the read command is re-issued, up
//!    to [`MAX_REREADS`] times, rather than proceeding on a guess. The benchmark's own runs
//!    never need it — a projection that obeys the contract above is scrapable, which is
//!    itself a finding — so [`Renderer::Flaky`] exists to fire it on purpose in the evidence,
//!    because an untested recovery path is a decoration.
//!
//! # What bn-2c0a's falsification campaign added here, and what it did not change
//!
//! The landed constructors are the landed baseline: [`ShellSurface::new`] is
//! [`Renderer::Standard`] under [`Disciplines::ALL`], and every scheduled run uses it. The
//! campaign needed three things this module did not have, and all three are additive.
//!
//! - [`Disciplines`] makes each of the four a knob, so "how much of the baseline's result is
//!   the *discipline* rather than the *interface*" can be answered with a number instead of
//!   an opinion. The answer is in `tests/dx10_falsification.rs`, attack N3: discipline 1
//!   carries all of it, and disciplines 2 and 4 are inert against the standard projection.
//! - [`Renderer::Stale`] and [`Renderer::Drifting`] are the two fault classes only a text
//!   surface can suffer — an answer one beat old, and an output format that moved between
//!   releases. Neither is in any scheduled run; both are attacks S4 and N2.
//! - [`HiddenCost`] records what the CLI spent underneath the agent — the frames and the
//!   per-invocation handshakes the second bullet at the top of this file says are free — so
//!   that handicap can be *priced* rather than merely named. It enters no
//!   [`Observation::bytes`], and `control_the_hidden_ledger_never_enters_an_interface_byte_total`
//!   is the test that holds it out.

use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::is_mutation;
use continuumd::protocol::envelope::{
    Budget, RequestEnvelope, ResultEnvelope, SemanticVerdictValue, Verdict,
};
use continuumd::protocol::operations::task::{
    TaskCancelRequest, TaskResumeRequest, TaskStatusRequest,
};
use continuumd::protocol::operations::verification::{
    VerificationResultRequest, VerificationStartRequest,
};
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateRequest, WorkspaceForkRequest, WorkspaceSealRequest,
};
use continuumd::protocol::scalar::{
    ContinuationHandle, Opaque, OperationName, RequestId, TaskHandle, WorkspaceHandle,
};
use continuumd::protocol::shared::{FileOverlay, Target};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    ErrorCode, Portfolio, ResultStatus, SemanticVerdict, TaskStatus,
};
use continuumd::transport::{decode_result, encode_arguments, encode_request};

use crate::policy::{Call, EDITED_MODULE, Step};
use crate::rig::Rig;
use crate::surface::{Arm, Observation, Reading, Surface, SurfaceError};
use crate::task::BenchmarkTask;

/// How many times a missing required key is re-read before the baseline gives up.
pub const MAX_REREADS: u32 = 2;

/// How the projection is produced.
///
/// [`Renderer::Standard`] obeys the projection contract. [`Renderer::Flaky`] withholds one
/// key from the *first* rendering of each command and then behaves normally, which is a fault
/// injected on purpose so that discipline 4 — bounded re-read — is exercised by evidence
/// rather than asserted in a comment. Nothing in the benchmark's scheduled runs uses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Renderer {
    /// The projection contract, unmodified.
    Standard,
    /// Withhold `key` from the first rendering of each command.
    Flaky {
        /// The key withheld.
        key: &'static str,
    },
    /// Answer the `at`th rendering from the *previous* command's answer.
    ///
    /// bn-2c0a's staleness attack, and nothing in the scheduled runs uses it. It models
    /// the subsidy the landed baseline receives by construction: this adapter renders from
    /// the very wire answer the daemon just produced, while a real CLI process re-derives
    /// state from a file, a cache, or a replica and can be a beat behind.
    Stale {
        /// Which rendering, one-based, answers from the held one.
        at: u32,
    },
    /// Rename `from` to `to` in every rendering.
    ///
    /// bn-2c0a's parse-drift attack: the fault class that can only land on the text arm,
    /// because the typed arm's field names are the schema's.
    Drifting {
        /// The key the projection used to print.
        from: &'static str,
        /// The key it prints now.
        to: &'static str,
    },
}

/// Which of the four disciplines this baseline is holding to.
///
/// [`Disciplines::ALL`] is the landed baseline and the default of every landed
/// constructor; nothing in the scheduled runs uses anything else. The knobs exist for
/// bn-2c0a's sensitivity bound: the goal bone's baseline is a *disciplined* scraper, and
/// "how much of the baseline's result is the discipline rather than the interface" is a
/// question that can only be answered by running the undisciplined one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disciplines {
    /// Discipline 1: match a line whose prefix is exactly `key: `. Off, the agent takes
    /// the first line that *contains* the key, which is how a scraper written in a hurry
    /// reads text.
    pub anchored_lookup: bool,
    /// Discipline 2: parse through [`ProtocolEnum::from_wire`], so an unrecognized token
    /// is a miss. Off, an unrecognized token becomes the value the agent expected.
    pub closed_vocabulary: bool,
    /// Discipline 3: expand a summarized list by command. Off, the agent assumes the
    /// declared ceiling was metered instead of paying to find out.
    pub explicit_expansion: bool,
    /// Discipline 4: re-read a missing required key, bounded. Off, the agent proceeds.
    pub bounded_reread: bool,
}

impl Disciplines {
    /// All four: the landed baseline.
    pub const ALL: Self = Self {
        anchored_lookup: true,
        closed_vocabulary: true,
        explicit_expansion: true,
        bounded_reread: true,
    };

    /// None: the scraper the goal bone's "disciplined" is a contrast with.
    pub const NONE: Self = Self {
        anchored_lookup: false,
        closed_vocabulary: false,
        explicit_expansion: false,
        bounded_reread: false,
    };
}

/// What one CLI process spent *underneath* the agent, and was not charged for.
///
/// Not an interface cost by this module's own accounting: these frames never reach the
/// agent, and the deliberate handicap they represent is stated at the top of this file.
/// They are recorded — and never added to [`Observation::bytes`] — because bn-2c0a's
/// falsification campaign has to price that handicap, and a handicap nobody measured is a
/// claim rather than a number. [`crate::variants`] is the only reader.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HiddenCost {
    /// Request-frame bytes the CLI process wrote to the daemon.
    pub frames_sent: u64,
    /// Result-frame bytes the CLI process read from the daemon.
    pub frames_received: u64,
    /// How many times a CLI process was invoked — one per command line written, bounded
    /// re-reads included. A real `continuum` binary handshakes once per invocation, which
    /// is what makes this number a cost rather than a curiosity.
    pub invocations: u32,
    /// Agent-interface bytes spent on discipline 3's expansion commands. Already inside
    /// [`Observation::bytes`]; broken out so a variant can re-price them.
    pub expansion_bytes: u64,
    /// How many expansion commands were issued. Each is a further process invocation a
    /// real CLI would have to make, and this harness serves it from an envelope it is
    /// already holding.
    pub expansions: u32,
    /// How many bounded re-reads discipline 4 performed.
    pub rereads: u32,
    /// [`structural_tokens`] of everything the agent wrote and read.
    pub agent_tokens: u64,
    /// [`structural_tokens`] of the CLI's own frames — the same canonical JSON the typed
    /// arm exchanges, which is what makes it a usable stand-in for the typed arm's density
    /// under a counting rule that is not affine in bytes.
    pub frame_tokens: u64,
}

impl HiddenCost {
    /// The CLI's own protocol traffic, both directions.
    #[must_use]
    pub const fn frame_bytes(&self) -> u64 {
        self.frames_sent + self.frames_received
    }
}

/// A counting rule that is **not** affine in bytes: words and punctuation, separately.
///
/// A token is a maximal run of `[A-Za-z0-9_]`, or one non-whitespace character that is not
/// in that class. Whitespace separates and costs nothing. It is a rule, not a tokenizer —
/// it has no vocabulary and no model — and its only purpose is to be a counting rule under
/// which dense canonical JSON and sparse prose do *not* cost the same per byte, so
/// `bytes/k`'s invariance can be checked against something rather than asserted.
#[must_use]
pub fn structural_tokens(text: &str) -> u64 {
    let mut tokens = 0;
    let mut in_word = false;
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            if !in_word {
                tokens += 1;
                in_word = true;
            }
        } else {
            in_word = false;
            if !character.is_whitespace() {
                tokens += 1;
            }
        }
    }
    tokens
}

/// The shell baseline: renders commands, reads text, and knows nothing else.
#[derive(Debug)]
pub struct ShellSurface {
    renderer: Renderer,
    /// How many renderings this surface has produced, so [`Renderer::Flaky`] can withhold
    /// only the first of each command.
    rendered: u32,
    /// Bytes the agent has written and read.
    bytes: u64,
    /// How many bounded re-reads discipline 4 has performed.
    rereads: u32,
    /// How many expansion commands discipline 3 has issued.
    expansions: u32,
    /// What this baseline spent that the agent was not charged for.
    hidden: HiddenCost,
    /// Which disciplines it is holding to.
    disciplines: Disciplines,
    /// The previous rendering's answer, for [`Renderer::Stale`].
    previous: Option<(ResultEnvelope, Payload)>,
    /// Every answer the CLI process received, when recording is on.
    ///
    /// Off by default and used only by [`crate::variants`]'s envelope decomposition, which
    /// needs the daemon's own answers to say which of the native arm's bytes a redesign
    /// could amortize and which the protocol requires.
    recorded: Vec<Recorded>,
    /// Whether to record.
    recording: bool,
}

/// One answer a CLI process received, kept for the envelope decomposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    /// The wire operation.
    pub operation: &'static str,
    /// The answer, as decoded from the frame.
    pub envelope: ResultEnvelope,
    /// Request-frame bytes.
    pub sent: u64,
    /// Result-frame bytes.
    pub received: u64,
    /// Bytes of the operation's own request struct inside the envelope's `arguments`.
    ///
    /// For `workspace.create` this is `SnapshotComponents` — the ten commitment lists a
    /// remote client must transmit where a local process names a directory — and it is
    /// recorded separately because that is the one request-side quantity a redesign could
    /// act on.
    pub argument_bytes: u64,
}

impl Default for ShellSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellSurface {
    /// A baseline with the standard projection.
    #[must_use]
    pub const fn new() -> Self {
        Self::with_renderer(Renderer::Standard)
    }

    /// A baseline whose projection withholds `key` from the first rendering of each command.
    #[must_use]
    pub const fn flaky(key: &'static str) -> Self {
        Self::with_renderer(Renderer::Flaky { key })
    }

    /// A baseline driving a named renderer, holding all four disciplines.
    #[must_use]
    pub const fn with_renderer(renderer: Renderer) -> Self {
        Self::configured(renderer, Disciplines::ALL)
    }

    /// A baseline driving a named renderer under a named set of disciplines.
    #[must_use]
    pub const fn configured(renderer: Renderer, disciplines: Disciplines) -> Self {
        Self {
            renderer,
            rendered: 0,
            bytes: 0,
            rereads: 0,
            expansions: 0,
            hidden: HiddenCost {
                frames_sent: 0,
                frames_received: 0,
                invocations: 0,
                expansion_bytes: 0,
                expansions: 0,
                rereads: 0,
                agent_tokens: 0,
                frame_tokens: 0,
            },
            disciplines,
            previous: None,
            recorded: Vec::new(),
            recording: false,
        }
    }

    /// The same baseline, recording every answer for the envelope decomposition.
    #[must_use]
    pub fn recording(mut self) -> Self {
        self.recording = true;
        self
    }

    /// Every answer this baseline's CLI process received, when recording is on.
    #[must_use]
    pub fn recorded(&self) -> &[Recorded] {
        &self.recorded
    }

    /// Which disciplines this baseline holds to.
    #[must_use]
    pub const fn disciplines(&self) -> Disciplines {
        self.disciplines
    }

    /// How many bounded re-reads this baseline has performed.
    #[must_use]
    pub const fn rereads(&self) -> u32 {
        self.rereads
    }

    /// How many expansion commands this baseline has issued.
    #[must_use]
    pub const fn expansions(&self) -> u32 {
        self.expansions
    }

    /// What this baseline spent that the agent was not charged for.
    #[must_use]
    pub const fn hidden(&self) -> HiddenCost {
        self.hidden
    }

    /// Drive one command: write it, get the daemon's answer, render it, read the text back.
    fn run_command(
        &mut self,
        rig: &mut Rig,
        task: &BenchmarkTask,
        step: &Step,
    ) -> Result<(String, String, ResultEnvelope, Payload), SurfaceError> {
        let command = command_line(task, step);
        let (envelope, payload, frames) = dispatch(rig, task, step, &command)?;
        self.hidden.frames_sent += frames.sent;
        self.hidden.frames_received += frames.received;
        self.hidden.frame_tokens += frames.tokens;
        self.hidden.invocations += 1;
        if self.recording {
            self.recorded.push(Recorded {
                operation: step.call.operation(),
                envelope: envelope.clone(),
                sent: frames.sent,
                received: frames.received,
                argument_bytes: frames.arguments,
            });
        }
        let text = self.render(&command, step.call.operation(), &envelope, &payload);
        Ok((command, text, envelope, payload))
    }

    fn render(
        &mut self,
        command: &str,
        operation: &str,
        envelope: &ResultEnvelope,
        payload: &Payload,
    ) -> String {
        self.rendered += 1;
        let withheld = match self.renderer {
            Renderer::Flaky { key } if self.rendered == 1 => Some(key),
            _ => None,
        };
        // A stale projection answers from the rendering *before* this one — a CLI process
        // reading a cache, a file, or a replica one operation behind. None of the four
        // disciplines can see it: every key is present, every token is in the closed
        // vocabulary, and every value is plausible. That is what the attack is for.
        let held = self.previous.replace((envelope.clone(), payload.clone()));
        let (envelope, payload) = match (self.renderer, held) {
            (Renderer::Stale { at }, Some(previous)) if self.rendered == at => previous,
            _ => (envelope.clone(), payload.clone()),
        };
        let text = render(command, operation, &envelope, &payload, withheld);
        match self.renderer {
            Renderer::Drifting { from, to } => drift(&text, from, to),
            _ => text,
        }
    }
}

/// Rename a projected key, as a CLI whose output format moved between releases would.
///
/// The value is untouched and the line is the same shape: only the *name* the agent
/// anchors on moves. A typed surface has no analogue — a field name belongs to the schema,
/// and a schema change is a protocol version.
#[must_use]
pub fn drift(text: &str, from: &str, to: &str) -> String {
    let prefix = format!("{from}: ");
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        match line.strip_prefix(prefix.as_str()) {
            Some(value) => {
                out.push_str(to);
                out.push_str(": ");
                out.push_str(value);
            }
            None => out.push_str(line),
        }
        out.push('\n');
    }
    out
}

impl Surface for ShellSurface {
    fn arm(&self) -> Arm {
        Arm::Shell
    }

    fn handshake_bytes(&self) -> u64 {
        // Opening a CLI session is not a thing an agent does: it runs a command. The
        // handshake a `continuum` process performs with the daemon is inside the process,
        // exactly like the frames it exchanges afterwards, and is not charged here for the
        // same reason those are not.
        0
    }

    fn perform(
        &mut self,
        rig: &mut Rig,
        task: &BenchmarkTask,
        step: &Step,
    ) -> Result<Observation, SurfaceError> {
        let before = self.bytes;
        let (command, mut text, envelope, _payload) = self.run_command(rig, task, step)?;
        self.bytes += command.len() as u64 + text.len() as u64;
        self.hidden.agent_tokens += structural_tokens(&command) + structural_tokens(&text);

        // Discipline 4: bounded re-read. A missing required key is re-read rather than
        // guessed at; the re-issued command and its output are charged like any other.
        let mut attempts = 0;
        while self.disciplines.bounded_reread
            && attempts < MAX_REREADS
            && missing_required(step.call.operation(), &text).is_some()
        {
            attempts += 1;
            self.rereads += 1;
            self.hidden.rereads += 1;
            let (again, retext, _, _) = self.run_command(rig, task, step)?;
            self.bytes += again.len() as u64 + retext.len() as u64;
            self.hidden.agent_tokens += structural_tokens(&again) + structural_tokens(&retext);
            text = retext;
        }

        let mut reading = scrape_with(step.call.operation(), &text, self.disciplines);

        // Discipline 3: explicit expansion. The count is in the summary; the manifest is not,
        // and the agent needs the manifest to know whether its declared ceiling was metered.
        if matches!(
            step.call,
            Call::StartVerification { .. } | Call::Resume { .. }
        ) && envelope.status != ResultStatus::Error
            && let Some(count) = reading.omissions
        {
            if self.disciplines.explicit_expansion {
                let expand = expansion_command(&envelope.request_id);
                let listing = render_omissions(&envelope);
                self.bytes += expand.len() as u64 + listing.len() as u64;
                self.hidden.agent_tokens +=
                    structural_tokens(&expand) + structural_tokens(&listing);
                self.expansions += 1;
                self.hidden.expansions += 1;
                self.hidden.expansion_bytes += expand.len() as u64 + listing.len() as u64;
                reading.ceiling_enforced =
                    Some(!listing.lines().any(|line| line == "budget.states"));
                // Under the standard projection the summary and the expansion are one
                // field and disagreeing would be a defect in this crate. Under
                // `Renderer::Stale` they are *meant* to disagree — the summary was read
                // off an answer one beat old and the expansion is served from the current
                // one, which is precisely the staleness bn-2c0a injects — so the check
                // holds where it is a check and stands aside where it is the experiment.
                debug_assert!(
                    !matches!(self.renderer, Renderer::Standard)
                        || count == listing.lines().filter(|line| !line.is_empty()).count(),
                    "the summary's count and the expansion's listing are one field"
                );
            } else {
                // Undisciplined: assume the ceiling was metered rather than pay to find
                // out. On this subset the assumption is always *correct*, which is the
                // finding — the disciplined baseline is charged for a datum it could have
                // guessed right on every task here.
                reading.ceiling_enforced = Some(true);
            }
        }

        let admitted = envelope.status != ResultStatus::Error;
        let code = envelope.error.value().map(|error| error.code);
        Ok(Observation {
            admitted,
            code,
            local_refusal: false,
            bytes: self.bytes - before,
            reading,
            line: transcript_line(step, admitted, code, self.bytes - before),
        })
    }
}

/// The canonical one-line transcript entry for one attempt.
///
/// Shared with the native arm through [`crate::run`]: it is the same function, so two arms'
/// transcripts differ exactly where their behaviour differs.
#[must_use]
pub fn transcript_line(step: &Step, admitted: bool, code: Option<ErrorCode>, bytes: u64) -> String {
    format!(
        "{} as={} admitted={} code={} bytes={}",
        step.call.operation(),
        step.principal.actor,
        admitted,
        code.map_or("-", ProtocolEnum::as_wire),
        bytes
    )
}

/// The command line an agent writes for one step.
#[must_use]
pub fn command_line(task: &BenchmarkTask, step: &Step) -> String {
    let principal = step.principal.actor;
    match &step.call {
        Call::CreateWorkspace { seal } => format!(
            "continuum workspace create --port {} --seal {seal} --as {principal}",
            task.source.port()
        ),
        Call::ForkModule { base } => format!(
            "continuum workspace fork --base {} --set {}=@edited --as {principal}",
            base.as_str(),
            task.source.module_path()
        ),
        Call::RestoreModule { base } => format!(
            "continuum workspace fork --base {} --set {}=@corpus --as {principal}",
            base.as_str(),
            task.source.module_path()
        ),
        Call::SealWorkspace { snapshot } => format!(
            "continuum workspace seal --snapshot {} --as {principal}",
            snapshot.as_str()
        ),
        Call::StartVerification { snapshot, states } => format!(
            "continuum verification start --snapshot {} --target {}:{} --states {states} \
             --wall-ms {} --bytes {} --as {principal}",
            snapshot.as_str(),
            task.target_kind.as_wire(),
            task.target_id,
            crate::declared_wall_ms(),
            crate::declared_bytes(),
        ),
        Call::PollTask { task: handle } => format!(
            "continuum task status --task {} --as {principal}",
            handle.as_str()
        ),
        Call::FetchResult { task: handle } => format!(
            "continuum verification result --task {} --as {principal}",
            handle.as_str()
        ),
        Call::Resume {
            continuation,
            states,
        } => format!(
            "continuum task resume --continuation {} --states {states} --wall-ms {} \
             --bytes {} --as {principal}",
            continuation.as_str(),
            crate::declared_wall_ms(),
            crate::declared_bytes(),
        ),
        Call::Cancel { task: handle } => format!(
            "continuum task cancel --task {} --as {principal}",
            handle.as_str()
        ),
    }
}

/// The expansion command discipline 3 issues.
#[must_use]
pub fn expansion_command(request: &RequestId) -> String {
    format!("continuum result omissions --request {}", request.as_str())
}

/// The typed request one step becomes.
///
/// Shared by both arms in spirit and by this one in fact: the native arm calls the client's
/// named operations, which build the identical structs. Two spellings of one call would let
/// the arms diverge in what they *asked for*, which is the one difference this benchmark must
/// not have.
#[must_use]
pub fn arguments_for(rig: &Rig, task: &BenchmarkTask, step: &Step) -> Arguments {
    match &step.call {
        Call::CreateWorkspace { seal } => Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: rig.components(task.source),
            overlay: Optional::Absent,
            seal: Optional::Present(*seal),
        }),
        Call::ForkModule { base } => Arguments::WorkspaceFork(WorkspaceForkRequest {
            base: base.clone(),
            overlay: Optional::Present(vec![FileOverlay {
                path: task.source.module_path().to_owned(),
                content: EDITED_MODULE.to_vec(),
            }]),
            patches: Optional::Absent,
        }),
        Call::RestoreModule { base } => Arguments::WorkspaceFork(WorkspaceForkRequest {
            base: base.clone(),
            overlay: Optional::Present(vec![FileOverlay {
                path: task.source.module_path().to_owned(),
                content: task.source.module().as_bytes().to_vec(),
            }]),
            patches: Optional::Absent,
        }),
        Call::SealWorkspace { snapshot } => Arguments::WorkspaceSeal(WorkspaceSealRequest {
            snapshot: snapshot.clone(),
        }),
        Call::StartVerification { .. } => Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: task.target_kind,
                id: task.target_id.to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
        Call::PollTask { task: handle } => Arguments::TaskStatus(TaskStatusRequest {
            task: handle.clone(),
        }),
        Call::FetchResult { task: handle } => {
            Arguments::VerificationResult(VerificationResultRequest {
                task: handle.clone(),
            })
        }
        Call::Resume {
            continuation,
            states,
        } => Arguments::TaskResume(TaskResumeRequest {
            continuation: continuation.clone(),
            budget: Optional::Present(crate::declared_budget(*states)),
        }),
        Call::Cancel { task: handle } => Arguments::TaskCancel(TaskCancelRequest {
            task: handle.clone(),
        }),
    }
}

/// Drive one step through the wire, as the CLI process would.
///
/// The frames this exchanges are the CLI's own and are not charged to the agent — see the
/// module documentation. What it returns is what the process has to render from, plus the
/// two frame lengths, which enter no interface-byte total and exist so
/// [`crate::variants`] can price the handicap that choice grants the baseline.
#[derive(Debug, Clone, Copy)]
struct FrameCost {
    sent: u64,
    received: u64,
    tokens: u64,
    arguments: u64,
}

fn dispatch(
    rig: &mut Rig,
    task: &BenchmarkTask,
    step: &Step,
    command: &str,
) -> Result<(ResultEnvelope, Payload, FrameCost), SurfaceError> {
    let operation = step.call.operation();
    let arguments = arguments_for(rig, task, step);
    let version = rig.version();
    let envelope = RequestEnvelope {
        protocol_version: version,
        // A CLI process derives a request identifier from the command it is running, which
        // is what makes a shell transcript reproducible. `rendered` is not used: the counter
        // lives on the surface and the identifier must be a function of the command alone.
        request_id: RequestId::new(&format!("req_s{:08x}", fnv1a(command.as_bytes())))
            .expect("a hexadecimal digest renders a well-formed request id"),
        idempotency_key: if is_mutation(operation) {
            Optional::Present(format!("idem-s{:08x}", fnv1a(command.as_bytes())))
        } else {
            Optional::Absent
        },
        actor: step.principal.actor_id(),
        capability: step.principal.capability_handle(),
        operation: OperationName::new(operation).expect("a registry name"),
        snapshot: match &step.call {
            Call::StartVerification { snapshot, .. } => Nullable::Value(snapshot.clone()),
            _ => Nullable::Null,
        },
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        // `verification.start` and `task.resume` are both `@task_starting`, and RFC 0026
        // makes the envelope's `budget` REQUIRED for those and only those. A resume
        // therefore declares its ceiling twice — once on the envelope, once in the request
        // body — because the IDL asks for both, and a CLI that filled in one would be
        // answered `MalformedRequest`.
        budget: match &step.call {
            Call::StartVerification { states, .. } | Call::Resume { states, .. } => {
                Optional::Present(crate::declared_budget(*states))
            }
            _ => Optional::Absent,
        },
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    };

    // The request frame is encoded here rather than inside `client_send` so its length can
    // be read off the same bytes that go on the wire — a measurement, not an estimate.
    // `client_send` would encode the identical frame; `sent_bytes_are_the_frame_client_send_
    // would_have_written` in the falsification evidence holds the two to each other.
    let frame = encode_request(&envelope, &arguments).map_err(|error| SurfaceError {
        arm: Arm::Shell,
        operation,
        detail: error.to_string(),
    })?;
    let sent = frame.len() as u64;
    let (server, pair, _) = rig.boundary();
    pair.to_server
        .put_frame(&frame)
        .map_err(|error| SurfaceError {
            arm: Arm::Shell,
            operation,
            detail: error.to_string(),
        })?;
    server.serve(pair).map_err(|error| SurfaceError {
        arm: Arm::Shell,
        operation,
        detail: error.to_string(),
    })?;
    let answer = pair
        .to_client
        .take_frame()
        .map_err(|error| SurfaceError {
            arm: Arm::Shell,
            operation,
            detail: error.to_string(),
        })?
        .ok_or_else(|| SurfaceError {
            arm: Arm::Shell,
            operation,
            detail: "the daemon wrote no answer frame".to_owned(),
        })?;
    let received = answer.len() as u64;
    let tokens = structural_tokens(&String::from_utf8_lossy(&frame))
        + structural_tokens(&String::from_utf8_lossy(&answer));
    let (result, payload) = decode_result(operation, &answer).map_err(|error| SurfaceError {
        arm: Arm::Shell,
        operation,
        detail: error.to_string(),
    })?;
    let arguments = encode_arguments(&arguments).map_or(0, |opaque| opaque.as_bytes().len() as u64);
    Ok((
        result,
        payload,
        FrameCost {
            sent,
            received,
            tokens,
            arguments,
        },
    ))
}

/// A stable 32-bit digest, used only to name a request after the command that produced it.
///
/// FNV-1a rather than BLAKE3 because this is a *tracing* identifier and RFC 0026 says so
/// outright ("a tracing identifier, never an artifact identity"). Nothing is authenticated by
/// it, nothing is content-addressed by it, and using the workspace's cryptographic identity
/// seam here would suggest otherwise.
const fn fnv1a(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        index += 1;
    }
    hash
}

// --- the projection -------------------------------------------------------------------

/// Project one answer into the text a human CLI prints.
#[must_use]
pub fn render(
    command: &str,
    operation: &str,
    envelope: &ResultEnvelope,
    payload: &Payload,
    withheld: Option<&str>,
) -> String {
    let mut lines: Vec<(String, String)> = Vec::new();
    lines.push(("status".to_owned(), envelope.status.as_wire().to_owned()));
    lines.push((
        "request".to_owned(),
        envelope.request_id.as_str().to_owned(),
    ));

    let mut task_handle = envelope
        .task
        .value()
        .map(TaskHandle::as_str)
        .map(str::to_owned);
    let mut continuation = envelope
        .continuation
        .value()
        .map(ContinuationHandle::as_str)
        .map(str::to_owned);

    match payload {
        Payload::WorkspaceCreate(response) => {
            lines.push(("snapshot".to_owned(), response.snapshot.as_str().to_owned()));
            lines.push(("sealed".to_owned(), response.sealed.to_string()));
            lines.push((
                "diagnostics".to_owned(),
                response.diagnostics.len().to_string(),
            ));
        }
        Payload::WorkspaceFork(response) => {
            lines.push(("snapshot".to_owned(), response.snapshot.as_str().to_owned()));
            lines.push(("sealed".to_owned(), "false".to_owned()));
            lines.push((
                "diagnostics".to_owned(),
                response.diagnostics.len().to_string(),
            ));
        }
        Payload::WorkspaceSeal(response) => {
            lines.push(("snapshot".to_owned(), response.snapshot.as_str().to_owned()));
            lines.push(("sealed".to_owned(), "true".to_owned()));
            lines.push((
                "root_digest".to_owned(),
                response.root_digest.as_str().to_owned(),
            ));
        }
        Payload::VerificationStart(response) => {
            if let Some(handle) = response.task.value() {
                task_handle = Some(handle.as_str().to_owned());
            }
        }
        Payload::TaskStatus(record) => {
            task_handle = Some(record.task.as_str().to_owned());
            lines.push(("task.status".to_owned(), record.status.as_wire().to_owned()));
            lines.push((
                "cost.states".to_owned(),
                record
                    .cost
                    .states
                    .value()
                    .map_or_else(|| "none".to_owned(), u64::to_string),
            ));
            lines.push(("milestones".to_owned(), record.milestones.len().to_string()));
            continuation = record
                .continuation
                .value()
                .map(ContinuationHandle::as_str)
                .map(str::to_owned)
                .or(continuation);
        }
        Payload::TaskResume(response) => {
            task_handle = Some(response.task.as_str().to_owned());
            lines.push((
                "task.status".to_owned(),
                response.status.as_wire().to_owned(),
            ));
        }
        Payload::TaskCancel(response) => {
            task_handle = Some(response.task.as_str().to_owned());
            lines.push((
                "task.status".to_owned(),
                response.status.as_wire().to_owned(),
            ));
            lines.push((
                "committed_evidence".to_owned(),
                response.committed_evidence.len().to_string(),
            ));
            if let Nullable::Value(handle) = &response.continuation {
                continuation = Some(handle.as_str().to_owned());
            }
        }
        Payload::VerificationResult(result) | Payload::VerificationAwait(result) => {
            task_handle = Some(result.task.as_str().to_owned());
            lines.push(("fragments".to_owned(), result.fragments.len().to_string()));
            lines.push((
                "crashpack".to_owned(),
                result
                    .crashpack
                    .value()
                    .map_or_else(|| "none".to_owned(), |handle| handle.as_str().to_owned()),
            ));
            if let Some(handle) = result.continuation.value() {
                continuation = Some(handle.as_str().to_owned());
            }
        }
        _ => {}
    }

    if let Some(semantic) = semantic_verdict(envelope) {
        lines.push(("verdict".to_owned(), semantic.verdict.as_wire().to_owned()));
        lines.push((
            "verdict.assurance".to_owned(),
            semantic.assurance_class.as_wire().to_owned(),
        ));
        lines.push((
            "verdict.inconclusive_reason".to_owned(),
            semantic
                .inconclusive_reason
                .value()
                .map_or("none", |reason| reason.as_wire())
                .to_owned(),
        ));
    }

    lines.push((
        "task".to_owned(),
        task_handle.unwrap_or_else(|| "none".to_owned()),
    ));
    lines.push((
        "continuation".to_owned(),
        continuation.unwrap_or_else(|| "none".to_owned()),
    ));

    if let Some(error) = envelope.error.value() {
        lines.push(("error.code".to_owned(), error.code.as_wire().to_owned()));
        lines.push(("error.detail".to_owned(), error.detail.clone()));
        lines.push(("error.retryable".to_owned(), error.retryable.to_string()));
        lines.push((
            "error.recovery".to_owned(),
            error.recovery.len().to_string(),
        ));
        lines.push((
            "error.non_resumable_reason".to_owned(),
            error
                .non_resumable_reason
                .value()
                .cloned()
                .unwrap_or_else(|| "none".to_owned()),
        ));
    }

    let omissions = envelope.omissions.len();
    lines.push((
        "omissions".to_owned(),
        if omissions == 0 {
            "0".to_owned()
        } else {
            format!(
                "{omissions} (expand: {})",
                expansion_command(&envelope.request_id)
            )
        },
    ));
    lines.push(("warnings".to_owned(), envelope.warnings.len().to_string()));
    lines.push((
        "next_operations".to_owned(),
        envelope.next_operations.len().to_string(),
    ));
    lines.push(("artifacts".to_owned(), envelope.artifacts.len().to_string()));
    let _ = operation;

    let mut text = format!("$ {command}\n");
    for (key, value) in lines {
        if withheld == Some(key.as_str()) {
            continue;
        }
        text.push_str(&key);
        text.push_str(": ");
        text.push_str(&value);
        text.push('\n');
    }
    text
}

/// The expansion listing: one omission subject per line, in the order the answer carries.
#[must_use]
pub fn render_omissions(envelope: &ResultEnvelope) -> String {
    let mut text = String::new();
    for omission in &envelope.omissions {
        text.push_str(&omission.subject);
        text.push('\n');
    }
    text
}

fn semantic_verdict(envelope: &ResultEnvelope) -> Option<&SemanticVerdictValue> {
    match envelope.verdict.value() {
        Some(Verdict::Semantic(value)) => Some(value),
        _ => None,
    }
}

// --- the agent half -------------------------------------------------------------------

/// Discipline 1: the value of `key`, matched on an anchored line prefix.
#[must_use]
pub fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}: ");
    text.lines()
        .find_map(|line| line.strip_prefix(prefix.as_str()))
}

/// The required keys of one operation's projection.
///
/// "Required" means the policy cannot proceed without it, not "the CLI happens to print it".
#[must_use]
pub fn required_keys(operation: &str) -> &'static [&'static str] {
    match operation {
        "workspace.create" | "workspace.fork" => &["status", "snapshot", "sealed"],
        "workspace.seal" => &["status", "snapshot", "sealed"],
        "verification.start" | "task.resume" => &["status", "task"],
        "task.status" => &["status", "task.status", "cost.states", "continuation"],
        "verification.result" => &["status", "verdict"],
        "task.cancel" => &["status", "task.status"],
        _ => &["status"],
    }
}

/// The first required key missing from `text`, when the answer was not an error.
///
/// An error answer is *expected* to be missing the payload keys: the operation did not run.
/// Re-reading one would be re-issuing a command that already answered, which is what
/// discipline 4 exists not to do.
#[must_use]
pub fn missing_required(operation: &str, text: &str) -> Option<&'static str> {
    if field(text, "status") == Some("error") {
        return None;
    }
    required_keys(operation)
        .iter()
        .copied()
        .find(|key| field(text, key).is_none())
}

/// Disciplines 1 and 2: recover what the text says, and nothing more.
#[must_use]
pub fn scrape(operation: &str, text: &str) -> Reading {
    scrape_with(operation, text, Disciplines::ALL)
}

/// Discipline 1, relaxed: the first line that merely *contains* `key`, split at `": "`.
///
/// What a scraper written without discipline 1 does, and it is wrong in exactly the way
/// discipline 1 exists to prevent: a lookup for `task` finds the `task.status:` line, and
/// a lookup for `code` finds `error.code:`.
#[must_use]
pub fn loose_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find(|line| line.contains(key))
        .and_then(|line| line.split_once(": "))
        .map(|(_, value)| value)
}

/// Recover what the text says, under a named set of disciplines.
#[must_use]
pub fn scrape_with(operation: &str, text: &str, disciplines: Disciplines) -> Reading {
    let look = |key: &str| -> Option<&str> {
        if disciplines.anchored_lookup {
            field(text, key)
        } else {
            loose_field(text, key)
        }
    };
    let mut reading = Reading::default();
    let errored = look("status") == Some("error");

    if !errored {
        reading.snapshot = look("snapshot").and_then(|value| WorkspaceHandle::new(value).ok());
        reading.sealed = match look("sealed") {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        };
        reading.task = look("task")
            .filter(|value| *value != "none")
            .and_then(|value| TaskHandle::new(value).ok());
        reading.status = look("task.status").and_then(|value| {
            TaskStatus::from_wire(value)
                .ok()
                // Discipline 2, relaxed: a token the closed vocabulary does not know
                // becomes the value the agent was expecting. A miss and a plausible wrong
                // answer are the same length in a transcript and are not the same fact.
                .or_else(|| (!disciplines.closed_vocabulary).then_some(TaskStatus::Completed))
        });
        reading.states = look("cost.states").and_then(|value| value.parse::<u64>().ok());
        reading.verdict = look("verdict").and_then(|value| {
            SemanticVerdict::from_wire(value)
                .ok()
                .or_else(|| (!disciplines.closed_vocabulary).then_some(SemanticVerdict::Refuted))
        });
        if let Some(value) = look("continuation") {
            reading.continuation_known = true;
            reading.continuation = (value != "none")
                .then(|| ContinuationHandle::new(value).ok())
                .flatten();
            if reading.continuation.is_none() && value != "none" {
                // A continuation that does not parse is not an absent one: the answer said
                // something and this arm could not read it. Saying "unknown" is the honest
                // reading and lets the policy re-read rather than drop a resumable campaign.
                reading.continuation_known = false;
            }
        }
    }
    // The summary line's count is scrapable whether or not the answer errored; the manifest
    // behind it is not, which is what the expansion command is for.
    reading.omissions = look("omissions").and_then(|value| {
        value
            .split_whitespace()
            .next()
            .and_then(|count| count.parse::<usize>().ok())
    });
    let _ = operation;
    reading
}

/// The budget a step declares, exposed so an evidence test can assert what the wire carried.
#[must_use]
pub fn declared(states: u64) -> Budget {
    crate::declared_budget(states)
}
