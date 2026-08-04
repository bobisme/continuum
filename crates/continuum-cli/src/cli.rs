//! Argument parsing and dispatch for the command groups this crate has landed:
//! `continuum context expand`, `continuum task status|resume|cancel` (bn-3tz60);
//! `continuum debug open|state`, `continuum repair begin|review`, `continuum evidence show`
//! (bn-1g7e4); and `continuum snapshot create|fork|seal`, `continuum check start|result|await`,
//! `continuum explain compile` (bn-3rqvm).
//!
//! A sibling PR-13 bone owns the cross-cutting output-contract infrastructure (bn-ybh1z);
//! [`run`] therefore recognizes exactly the noun groups the three command bones delivered and
//! reports every other first word as a usage error rather than guessing at a verb no module
//! here owns.
//!
//! # Every command takes its handles on the command line
//!
//! There is no `--session`, no state file, no "current" task, branch, transaction, or
//! evidence node anywhere in this parser — INV-002. Two invocations of the same command
//! line mean the same thing on any terminal on any day, and a handle a caller does not type
//! is a handle no command uses.
//!
//! Flags are parsed by one small `--name value` / `--flag` scanner ([`Scan`]) rather than a
//! declarative parser: the surface is fifteen commands and two dozen flags, and a hand-rolled
//! scan keeps this crate's one external-facing dependency at `continuumd` (see the crate
//! root and `Cargo.toml` for why that edge is minimal on purpose this wave). The scanner
//! keeps both readings of a repeated `--name value`: the last occurrence ([`Scan::required`],
//! for a single-valued field) and all of them in order ([`Scan::all`], for `list<…>` fields
//! like `overlay`, `patches` and `guarantees`).

use continuumd::codec;
use continuumd::protocol::scalar::{
    ActorId, ArtifactHandle, CapabilityHandle, Commitment, ContextHandle, ContinuationHandle,
    CrashpackHandle, DebugHandle, EvidenceHandle, ProtocolVersion, RepairHandle, TaskHandle,
    WorkspaceHandle,
};
use continuumd::protocol::shared::{FileOverlay, SnapshotComponents, Target};
use continuumd::protocol::spec::{Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    Audience, ExpansionRelation, GateProfile, Portfolio, PriorityClass, TargetKind,
};

use crate::check::{self, AwaitArgs, ResultArgs, StartArgs};
use crate::context::{self, ExpandArgs};
use crate::debug::{self, OpenArgs, StateArgs};
use crate::error::CliError;
use crate::evidence::{self, ShowArgs};
use crate::explain::{self, CompileArgs};
use crate::format::{self, Format};
use crate::render::Rendered;
use crate::repair::{self, BeginArgs, ReviewArgs};
use crate::snapshot::{self, CreateArgs, ForkArgs, SealArgs};
use crate::task;
use crate::wire::{Connection, Transport};

/// The default protocol version a connection speaks when `--protocol-version` is not given
/// — the version every fixture and test rig in this workspace negotiates today
/// (`continuum-mcp/tests/typed_surface.rs`, `continuum-benchmark::rig::VERSION`).
pub const DEFAULT_PROTOCOL_VERSION: (u32, u32) = (3, 2);

/// Run one invocation: parse `args` (as given on the command line, program name already
/// stripped), make whatever wire call the verb needs over `transport`, and return the
/// rendered answer.
///
/// # Errors
///
/// [`CliError::Usage`] when the command line does not parse. [`CliError::Connection`] when
/// a wire call fails below the protocol. A daemon's typed refusal is never an error here —
/// see [`Rendered`].
pub fn run(
    args: &[String],
    transport: &mut dyn Transport,
    stdout_is_tty: bool,
) -> Result<Rendered, CliError> {
    let scan = Scan::of(args)?;
    let format = resolve_format(&scan, stdout_is_tty)?;
    let mut connection = build_connection(&scan)?;

    match scan.positional.first().map(String::as_str) {
        Some("snapshot") => dispatch_snapshot(&scan, &mut connection, transport, format),
        Some("check") => dispatch_check(&scan, &mut connection, transport, format),
        Some("explain") => dispatch_explain(&scan, &mut connection, transport, format),
        Some("context") => dispatch_context(&scan, &mut connection, transport, format),
        Some("task") => dispatch_task(&scan, &mut connection, transport, format),
        Some("debug") => dispatch_debug(&scan, &mut connection, transport, format),
        Some("repair") => dispatch_repair(&scan, &mut connection, transport, format),
        Some("evidence") => dispatch_evidence(&scan, &mut connection, transport, format),
        Some(other) => Err(CliError::usage(format!(
            "unknown command {other:?}; expected one of {}",
            NOUNS.join(", ")
        ))),
        None => Err(CliError::usage(USAGE)),
    }
}

/// The noun groups this build recognizes, in the order [`run`] matches them.
///
/// A named constant so the "unknown command" message and this module's tests read the same
/// list, rather than a sentence in one place drifting from a `match` in another.
pub const NOUNS: [&str; 8] = [
    "snapshot", "check", "explain", "context", "task", "debug", "repair", "evidence",
];

/// The one-line usage this build answers a bare `continuum` with.
const USAGE: &str = "usage: continuum snapshot create --components <json> \
     | continuum snapshot fork <ws_*> | continuum snapshot seal <ws_*> \
     | continuum check start <ws_*> --target <id> --target-kind <kind> --portfolio <p> \
       --states <n> \
     | continuum check result <task_*> | continuum check await <task_*> --timeout-ms <n> \
     | continuum explain compile --evidence-root <handle> --question <text> --bytes <n> \
     | continuum context expand ... \
     | continuum task status|resume|cancel <handle> \
     | continuum debug open <artifact> | continuum debug state <dbg_*> \
     | continuum repair begin <crash_*> --gate-profile <profile> \
     | continuum repair review <rt_*> \
     | continuum evidence show <ev_*>";

fn dispatch_snapshot(
    scan: &Scan,
    connection: &mut Connection,
    transport: &mut dyn Transport,
    format: Format,
) -> Result<Rendered, CliError> {
    let verb = scan.positional.get(1).map(String::as_str);
    let handle = scan.positional.get(2).map(String::as_str);
    match (verb, handle) {
        (Some("create"), _) => {
            let args = CreateArgs {
                components: components(scan)?,
                overlay: overlays(scan)?,
                seal: scan.seal_flag,
            };
            snapshot::create(connection, transport, &args, format)
        }
        (Some("fork"), Some(handle)) => {
            let args = ForkArgs {
                base: workspace_handle(handle)?,
                overlay: overlays(scan)?,
                patches: patches(scan),
            };
            snapshot::fork(connection, transport, &args, format)
        }
        (Some("seal"), Some(handle)) => {
            let args = SealArgs {
                snapshot: workspace_handle(handle)?,
            };
            snapshot::seal(connection, transport, &args, format)
        }
        (Some(verb @ ("fork" | "seal")), None) => Err(CliError::usage(format!(
            "\"snapshot {verb}\" requires a snapshot handle: this command takes no ambient \
             session and remembers no snapshot (INV-002)"
        ))),
        (Some(other), _) => Err(CliError::usage(format!(
            "unknown \"snapshot\" verb {other:?}; expected \"create\", \"fork\", or \"seal\""
        ))),
        (None, _) => Err(CliError::usage(
            "usage: continuum snapshot create --components <json> \
             | continuum snapshot fork <ws_*> | continuum snapshot seal <ws_*>",
        )),
    }
}

fn dispatch_check(
    scan: &Scan,
    connection: &mut Connection,
    transport: &mut dyn Transport,
    format: Format,
) -> Result<Rendered, CliError> {
    let verb = scan.positional.get(1).map(String::as_str);
    let handle = scan.positional.get(2).map(String::as_str);
    match (verb, handle) {
        (Some("start"), Some(handle)) => {
            let args = start_args(scan, handle)?;
            check::start(connection, transport, &args, format)
        }
        (Some("result"), Some(handle)) => {
            let args = ResultArgs {
                task: task_handle(handle)?,
            };
            check::result(connection, transport, &args, format)
        }
        (Some("await"), Some(handle)) => {
            let args = AwaitArgs {
                task: task_handle(handle)?,
                // Required, never defaulted: an unbounded wait is the one thing this
                // command must not perform, and the bound is the caller's to choose.
                timeout_ms: scan.required_u64("timeout-ms")?,
            };
            check::await_result(connection, transport, &args, format)
        }
        (Some("start"), None) => Err(CliError::usage(
            "\"check start\" requires the sealed snapshot the campaign runs over (INV-002)",
        )),
        (Some(verb @ ("result" | "await")), None) => Err(CliError::usage(format!(
            "\"check {verb}\" requires a task handle: this command takes no ambient session \
             and remembers no task (INV-002)"
        ))),
        (Some(other), _) => Err(CliError::usage(format!(
            "unknown \"check\" verb {other:?}; expected \"start\", \"result\", or \"await\""
        ))),
        (None, _) => Err(CliError::usage(
            "usage: continuum check start <ws_*> --target <id> --target-kind <kind> \
             --portfolio <portfolio> --states <n> \
             | continuum check result <task_*> \
             | continuum check await <task_*> --timeout-ms <n>",
        )),
    }
}

fn dispatch_explain(
    scan: &Scan,
    connection: &mut Connection,
    transport: &mut dyn Transport,
    format: Format,
) -> Result<Rendered, CliError> {
    match scan.positional.get(1).map(String::as_str) {
        Some("compile") => {
            let args = compile_args(scan)?;
            explain::compile(connection, transport, &args, format)
        }
        Some(other) => Err(CliError::usage(format!(
            "unknown \"explain\" verb {other:?}; expected \"compile\""
        ))),
        None => Err(CliError::usage(
            "usage: continuum explain compile --evidence-root <handle> --question <text> \
             --bytes <n>",
        )),
    }
}

fn dispatch_context(
    scan: &Scan,
    connection: &mut Connection,
    transport: &mut dyn Transport,
    format: Format,
) -> Result<Rendered, CliError> {
    match scan.positional.get(1).map(String::as_str) {
        Some("expand") => {
            let args = expand_args(scan)?;
            context::expand(connection, transport, &args, format)
        }
        Some(other) => Err(CliError::usage(format!(
            "unknown \"context\" verb {other:?}; expected \"expand\""
        ))),
        None => Err(CliError::usage(
            "usage: continuum context expand --context <ctx_*> --anchor <text> --relation <relation>",
        )),
    }
}

fn dispatch_task(
    scan: &Scan,
    connection: &mut Connection,
    transport: &mut dyn Transport,
    format: Format,
) -> Result<Rendered, CliError> {
    let verb = scan.positional.get(1).map(String::as_str);
    let handle = scan.positional.get(2).map(String::as_str);
    match (verb, handle) {
        (Some("status"), Some(handle)) => {
            let task = TaskHandle::new(handle).map_err(|error| {
                CliError::usage(format!("{handle:?} is not a task handle: {error}"))
            })?;
            task::status(connection, transport, &task, format)
        }
        (Some("cancel"), Some(handle)) => {
            let task = TaskHandle::new(handle).map_err(|error| {
                CliError::usage(format!("{handle:?} is not a task handle: {error}"))
            })?;
            task::cancel(connection, transport, &task, format)
        }
        (Some("resume"), Some(handle)) => {
            let continuation = ContinuationHandle::new(handle).map_err(|error| {
                CliError::usage(format!("{handle:?} is not a continuation handle: {error}"))
            })?;
            let states = scan.required_u64("states")?;
            task::resume(connection, transport, &continuation, states, format)
        }
        (Some(verb @ ("status" | "cancel" | "resume")), None) => Err(CliError::usage(format!(
            "\"task {verb}\" requires a handle"
        ))),
        (Some(other), _) => Err(CliError::usage(format!(
            "unknown \"task\" verb {other:?}; expected \"status\", \"resume\", or \"cancel\""
        ))),
        (None, _) => Err(CliError::usage(
            "usage: continuum task status|resume|cancel <handle>",
        )),
    }
}

fn dispatch_debug(
    scan: &Scan,
    connection: &mut Connection,
    transport: &mut dyn Transport,
    format: Format,
) -> Result<Rendered, CliError> {
    let verb = scan.positional.get(1).map(String::as_str);
    let handle = scan.positional.get(2).map(String::as_str);
    match (verb, handle) {
        (Some("open"), Some(handle)) => {
            let subject = ArtifactHandle::new(handle).map_err(|error| {
                CliError::usage(format!("{handle:?} is not an artifact handle: {error}"))
            })?;
            let args = OpenArgs {
                subject,
                observer: scan.flags.get("observer").cloned(),
            };
            debug::open(connection, transport, &args, format)
        }
        (Some("state"), Some(handle)) => {
            let branch = DebugHandle::new(handle).map_err(|error| {
                CliError::usage(format!("{handle:?} is not a debug branch handle: {error}"))
            })?;
            let args = StateArgs {
                branch,
                observer: scan.flags.get("observer").cloned(),
            };
            debug::state(connection, transport, &args, format)
        }
        (Some(verb @ ("open" | "state")), None) => Err(CliError::usage(format!(
            "\"debug {verb}\" requires a handle: this command takes no ambient session and \
             remembers no branch (INV-002)"
        ))),
        (Some(other), _) => Err(CliError::usage(format!(
            "unknown \"debug\" verb {other:?}; expected \"open\" or \"state\""
        ))),
        (None, _) => Err(CliError::usage(
            "usage: continuum debug open <artifact> | continuum debug state <dbg_*>",
        )),
    }
}

fn dispatch_repair(
    scan: &Scan,
    connection: &mut Connection,
    transport: &mut dyn Transport,
    format: Format,
) -> Result<Rendered, CliError> {
    let verb = scan.positional.get(1).map(String::as_str);
    let handle = scan.positional.get(2).map(String::as_str);
    match (verb, handle) {
        (Some("begin"), Some(handle)) => {
            let failure = CrashpackHandle::new(handle).map_err(|error| {
                CliError::usage(format!("{handle:?} is not a crashpack handle: {error}"))
            })?;
            // Required, never defaulted: the field is `required` in the IDL and the
            // vocabulary's member spelled `default` is one of the four choices, not the
            // absence of a choice. See `crate::repair`'s module doc.
            let profile = scan.required("gate-profile")?;
            let gate_profile = GateProfile::from_wire(&profile).map_err(|_| {
                CliError::usage(format!(
                    "{profile:?} is not a gate profile ({})",
                    GateProfile::ALL
                        .iter()
                        .map(|member| member.as_wire())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?;
            let args = BeginArgs {
                failure,
                gate_profile,
            };
            repair::begin(connection, transport, &args, format)
        }
        (Some("review"), Some(handle)) => {
            let repair = RepairHandle::new(handle).map_err(|error| {
                CliError::usage(format!(
                    "{handle:?} is not a repair transaction handle: {error}"
                ))
            })?;
            repair::review(connection, transport, &ReviewArgs { repair }, format)
        }
        (Some(verb @ ("begin" | "review")), None) => Err(CliError::usage(format!(
            "\"repair {verb}\" requires a handle: this command takes no ambient session and \
             remembers no transaction (INV-002)"
        ))),
        (Some(other), _) => Err(CliError::usage(format!(
            "unknown \"repair\" verb {other:?}; expected \"begin\" or \"review\""
        ))),
        (None, _) => Err(CliError::usage(
            "usage: continuum repair begin <crash_*> --gate-profile <profile> \
             | continuum repair review <rt_*>",
        )),
    }
}

fn dispatch_evidence(
    scan: &Scan,
    connection: &mut Connection,
    transport: &mut dyn Transport,
    format: Format,
) -> Result<Rendered, CliError> {
    let verb = scan.positional.get(1).map(String::as_str);
    let handle = scan.positional.get(2).map(String::as_str);
    match (verb, handle) {
        (Some("show"), Some(handle)) => {
            let evidence = EvidenceHandle::new(handle).map_err(|error| {
                CliError::usage(format!("{handle:?} is not an evidence handle: {error}"))
            })?;
            let args = ShowArgs {
                evidence,
                inline: scan.inline_flag,
            };
            evidence::show(connection, transport, &args, format)
        }
        (Some("show"), None) => Err(CliError::usage(
            "\"evidence show\" requires a handle: this command takes no ambient session and \
             remembers no node (INV-002)",
        )),
        (Some(other), _) => Err(CliError::usage(format!(
            "unknown \"evidence\" verb {other:?}; expected \"show\""
        ))),
        (None, _) => Err(CliError::usage("usage: continuum evidence show <ev_*>")),
    }
}

/// The `SnapshotComponents` document `--components` carries.
///
/// Decoded here, before any frame exists, so a malformed declaration is a usage error the
/// caller can fix rather than a daemon refusal they have to interpret. The document is
/// *canonical* JSON — the encoding the protocol defines, with keys in ascending code-point
/// order — because that is what `continuumd::codec` reads and this crate restates no second
/// spelling of the wire format.
fn components(scan: &Scan) -> Result<SnapshotComponents, CliError> {
    let document = scan.required("components")?;
    codec::from_bytes::<SnapshotComponents>(document.as_bytes()).map_err(|error| {
        CliError::usage(format!(
            "--components is not a canonical JSON SnapshotComponents document (keys in \
             ascending code-point order): {error}"
        ))
    })
}

/// Every `--overlay <path>=<bytes>` on the line, in order.
///
/// The one place in this crate where content rather than a content identity travels, because
/// `FileOverlay.content` is the one wire field that carries bytes (see
/// [`crate::snapshot`]'s module doc). The split is on the *first* `=`, so a path may not
/// contain one and content may contain as many as it likes.
fn overlays(scan: &Scan) -> Result<Vec<FileOverlay>, CliError> {
    scan.all("overlay")
        .iter()
        .map(|entry| {
            let (path, content) = entry
                .split_once('=')
                .ok_or_else(|| CliError::usage("--overlay takes <path>=<content>".to_owned()))?;
            Ok(FileOverlay {
                path: path.to_owned(),
                content: content.as_bytes().to_vec(),
            })
        })
        .collect()
}

/// Every `--patch <commitment>` on the line, in order.
///
/// No validation beyond "it is a string": `Commitment::new` is infallible because "the IDL
/// declares no pattern" for a content commitment, and a shape check invented here would be
/// this crate holding the wire to a rule the protocol does not state.
fn patches(scan: &Scan) -> Vec<Commitment> {
    scan.all("patch")
        .iter()
        .map(|token| Commitment::new(token))
        .collect()
}

fn workspace_handle(token: &str) -> Result<WorkspaceHandle, CliError> {
    WorkspaceHandle::new(token)
        .map_err(|error| CliError::usage(format!("{token:?} is not a snapshot handle: {error}")))
}

fn task_handle(token: &str) -> Result<TaskHandle, CliError> {
    TaskHandle::new(token)
        .map_err(|error| CliError::usage(format!("{token:?} is not a task handle: {error}")))
}

fn start_args(scan: &Scan, snapshot: &str) -> Result<StartArgs, CliError> {
    let kind = scan.required("target-kind")?;
    let kind = TargetKind::from_wire(&kind).map_err(|_| {
        CliError::usage(format!(
            "{kind:?} is not a target kind ({})",
            wire_members(TargetKind::ALL)
        ))
    })?;
    let portfolio = scan.required("portfolio")?;
    let portfolio = Portfolio::from_wire(&portfolio).map_err(|_| {
        CliError::usage(format!(
            "{portfolio:?} is not a portfolio ({})",
            wire_members(Portfolio::ALL)
        ))
    })?;
    let priority_class = match scan.flags.get("priority-class") {
        Some(token) => Optional::Present(PriorityClass::from_wire(token).map_err(|_| {
            CliError::usage(format!(
                "{token:?} is not a priority class ({})",
                wire_members(PriorityClass::ALL)
            ))
        })?),
        None => Optional::Absent,
    };
    Ok(StartArgs {
        snapshot: workspace_handle(snapshot)?,
        target: Target {
            kind,
            id: scan.required("target")?,
        },
        portfolio,
        priority_class,
        // Required, never defaulted: `verification.start` is `@task_starting`, so RFC 0026
        // requires a budget, and choosing a state ceiling for the caller would be this crate
        // deciding how much of a state space is enough.
        states: scan.required_u64("states")?,
    })
}

fn compile_args(scan: &Scan) -> Result<CompileArgs, CliError> {
    let evidence_root = scan.required("evidence-root")?;
    let evidence_root = ArtifactHandle::new(&evidence_root).map_err(|error| {
        CliError::usage(format!(
            "{evidence_root:?} is not an artifact handle: {error}"
        ))
    })?;
    let audience = match scan.flags.get("audience") {
        Some(token) => Optional::Present(Audience::from_wire(token).map_err(|_| {
            CliError::usage(format!(
                "{token:?} is not an audience ({})",
                wire_members(Audience::ALL)
            ))
        })?),
        None => Optional::Absent,
    };
    Ok(CompileArgs {
        evidence_root,
        question: scan.required("question")?,
        audience,
        guarantees: scan.all("guarantee").to_vec(),
        bytes: scan.required_u64("bytes")?,
    })
}

/// A closed vocabulary's members, comma-separated, for a usage error that names what *is*
/// accepted rather than only what is not.
fn wire_members<T: ProtocolEnum + Copy>(members: &[T]) -> String {
    members
        .iter()
        .map(|member| member.as_wire())
        .collect::<Vec<_>>()
        .join(", ")
}

fn expand_args(scan: &Scan) -> Result<ExpandArgs, CliError> {
    let context = scan.required("context")?;
    let context = ContextHandle::new(&context).map_err(|error| {
        CliError::usage(format!("{context:?} is not a context handle: {error}"))
    })?;
    let anchor = scan.required("anchor")?;
    let relation = scan.required("relation")?;
    let relation = ExpansionRelation::from_wire(&relation).map_err(|_| {
        CliError::usage(format!(
            "{relation:?} is not a known expansion relation ({})",
            ExpansionRelation::ALL
                .iter()
                .map(|member| member.as_wire())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;
    let depth = scan.optional_u32("depth")?;
    let states = scan.optional_u64("states")?;
    Ok(ExpandArgs {
        context,
        anchor,
        relation,
        depth,
        states,
    })
}

fn resolve_format(scan: &Scan, stdout_is_tty: bool) -> Result<Format, CliError> {
    let explicit = if scan.json_flag {
        Some(Format::Json)
    } else if let Some(token) = scan.flags.get("format") {
        Some(Format::parse(token)?)
    } else {
        None
    };
    let env = std::env::var("FORMAT").ok();
    Ok(format::resolve(explicit, env.as_deref(), stdout_is_tty))
}

fn build_connection(scan: &Scan) -> Result<Connection, CliError> {
    let actor = scan.required("actor")?;
    let actor = ActorId::new(&actor)
        .map_err(|error| CliError::usage(format!("{actor:?} is not an actor id: {error}")))?;
    let capability = scan.required("capability")?;
    let capability = CapabilityHandle::new(&capability).map_err(|error| {
        CliError::usage(format!(
            "{capability:?} is not a capability handle: {error}"
        ))
    })?;
    let version = match scan.flags.get("protocol-version") {
        Some(token) => parse_version(token)?,
        None => ProtocolVersion::new(DEFAULT_PROTOCOL_VERSION.0, DEFAULT_PROTOCOL_VERSION.1),
    };
    Ok(Connection::new(version, actor, capability)
        .with_idempotency_key(scan.flags.get("idempotency-key").cloned()))
}

fn parse_version(token: &str) -> Result<ProtocolVersion, CliError> {
    let (major, minor) = token.split_once('.').ok_or_else(|| {
        CliError::usage(format!(
            "{token:?} is not a protocol version (expected MAJOR.MINOR)"
        ))
    })?;
    let major: u32 = major.parse().map_err(|_| {
        CliError::usage(format!(
            "{token:?} is not a protocol version (expected MAJOR.MINOR)"
        ))
    })?;
    let minor: u32 = minor.parse().map_err(|_| {
        CliError::usage(format!(
            "{token:?} is not a protocol version (expected MAJOR.MINOR)"
        ))
    })?;
    Ok(ProtocolVersion::new(major, minor))
}

/// The command line, scanned once into positional words and `--name value` flags.
///
/// Three boolean flags exist and they are named in [`BOOLEAN_FLAGS`]: `--json`
/// (cli-conventions.md's hidden alias for `--format json`), `--inline`
/// (`evidence.get`'s `inline: Bool optional`), and `--seal` (`workspace.create`'s
/// `seal: Bool optional`). Every other `--name` consumes the following word as its value,
/// and a `--name` at the end of the line with nothing after it is a usage error rather than
/// a silently empty value.
struct Scan {
    positional: Vec<String>,
    flags: std::collections::BTreeMap<String, String>,
    /// Every occurrence of every `--name value` flag, in command-line order.
    ///
    /// [`Scan::flags`] keeps the last one, which is what a single-valued flag means; this
    /// keeps all of them, which is what `--overlay`, `--patch` and `--guarantee` mean. Both
    /// are filled by the same pass, so a flag cannot be repeatable in one arm and
    /// last-wins in another: which reading applies is a property of the *call site* that
    /// asks, and asking for the wrong one is visible in one line rather than in a parser.
    repeats: std::collections::BTreeMap<String, Vec<String>>,
    json_flag: bool,
    inline_flag: bool,
    seal_flag: bool,
}

/// The flags that take no value.
///
/// A table rather than a chain of `if`s so that [`Scan::of`] cannot know about a boolean
/// flag one arm forgot: a name here is value-less everywhere, and a name absent from it
/// consumes the next word everywhere.
const BOOLEAN_FLAGS: [&str; 3] = ["json", "inline", "seal"];

impl Scan {
    fn of(args: &[String]) -> Result<Self, CliError> {
        let mut positional = Vec::new();
        let mut flags = std::collections::BTreeMap::new();
        let mut repeats: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        let mut json_flag = false;
        let mut inline_flag = false;
        let mut seal_flag = false;
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            if let Some(name) = arg.strip_prefix("--") {
                if BOOLEAN_FLAGS.contains(&name) {
                    match name {
                        "json" => json_flag = true,
                        "inline" => inline_flag = true,
                        _ => seal_flag = true,
                    }
                    continue;
                }
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::usage(format!("--{name} requires a value")))?;
                flags.insert(name.to_owned(), value.clone());
                repeats
                    .entry(name.to_owned())
                    .or_default()
                    .push(value.clone());
            } else {
                positional.push(arg.clone());
            }
        }
        Ok(Self {
            positional,
            flags,
            repeats,
            json_flag,
            inline_flag,
            seal_flag,
        })
    }

    fn required(&self, name: &str) -> Result<String, CliError> {
        self.flags
            .get(name)
            .cloned()
            .ok_or_else(|| CliError::usage(format!("--{name} is required")))
    }

    /// Every occurrence of a repeatable flag, in command-line order. Empty when absent.
    fn all(&self, name: &str) -> &[String] {
        self.repeats.get(name).map_or(&[], Vec::as_slice)
    }

    fn optional_u32(&self, name: &str) -> Result<Optional<u32>, CliError> {
        match self.flags.get(name) {
            Some(text) => text
                .parse::<u32>()
                .map(Optional::Present)
                .map_err(|_| CliError::usage(format!("--{name} must be a non-negative integer"))),
            None => Ok(Optional::Absent),
        }
    }

    fn optional_u64(&self, name: &str) -> Result<Optional<u64>, CliError> {
        match self.flags.get(name) {
            Some(text) => text
                .parse::<u64>()
                .map(Optional::Present)
                .map_err(|_| CliError::usage(format!("--{name} must be a non-negative integer"))),
            None => Ok(Optional::Absent),
        }
    }

    fn required_u64(&self, name: &str) -> Result<u64, CliError> {
        self.required(name)?
            .parse::<u64>()
            .map_err(|_| CliError::usage(format!("--{name} must be a non-negative integer")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::LinkError;

    struct Deaf;
    impl Transport for Deaf {
        fn exchange(&mut self, _frame: &[u8]) -> Result<Vec<u8>, LinkError> {
            Err(LinkError::NoAnswer)
        }
    }

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn an_unknown_top_level_command_is_a_usage_error_not_a_wire_call() {
        // A first word no noun group owns. It was `"snapshot"` until bn-3rqvm landed that
        // group; a test whose "unknown" example became a real command would have gone on
        // passing for the wrong reason, so it names one no build has.
        let mut transport = Deaf;
        let error = run(
            &args(&["promote", "--actor", "agent:x", "--capability", "cap_x"]),
            &mut transport,
            false,
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 1);
        let CliError::Usage(detail) = &error else {
            panic!("an unknown command is a usage error: {error:?}");
        };
        for noun in NOUNS {
            assert!(detail.contains(noun), "the error names {noun}: {detail}");
        }
    }

    #[test]
    fn the_new_verb_groups_reject_a_verb_they_do_not_own() {
        for (noun, expected) in [
            ("snapshot", "\"create\", \"fork\", or \"seal\""),
            ("check", "\"start\", \"result\", or \"await\""),
            ("explain", "\"compile\""),
        ] {
            let mut transport = Deaf;
            let error = run(
                &args(&[
                    noun,
                    "promote",
                    "--actor",
                    "agent:x",
                    "--capability",
                    "cap_x",
                ]),
                &mut transport,
                false,
            )
            .unwrap_err();
            let CliError::Usage(detail) = &error else {
                panic!("an unknown verb is a usage error: {error:?}");
            };
            assert!(detail.contains(expected), "{noun}: {detail}");
        }
    }

    #[test]
    fn a_repeated_flag_keeps_every_occurrence_and_a_single_valued_one_keeps_the_last() {
        // The two readings live in one scan, so a call site asks for the one its wire field
        // declares rather than the parser deciding for it.
        let scan = Scan::of(&args(&[
            "--overlay",
            "a=1",
            "--overlay",
            "b=2",
            "--question",
            "first",
            "--question",
            "second",
        ]))
        .expect("the line parses");
        assert_eq!(scan.all("overlay"), ["a=1".to_owned(), "b=2".to_owned()]);
        assert_eq!(
            scan.flags.get("question").map(String::as_str),
            Some("second")
        );
        assert!(scan.all("patch").is_empty());
    }

    #[test]
    fn the_seal_flag_takes_no_value_and_does_not_swallow_the_next_word() {
        let scan = Scan::of(&args(&["snapshot", "create", "--seal", "--bytes", "16"]))
            .expect("the line parses");
        assert!(scan.seal_flag);
        assert_eq!(scan.flags.get("bytes").map(String::as_str), Some("16"));
    }

    #[test]
    fn task_status_without_a_handle_is_a_usage_error() {
        let mut transport = Deaf;
        let error = run(
            &args(&[
                "task",
                "status",
                "--actor",
                "agent:x",
                "--capability",
                "cap_x",
            ]),
            &mut transport,
            false,
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn a_malformed_task_handle_is_a_usage_error_and_never_reaches_the_wire() {
        let mut transport = Deaf;
        let error = run(
            &args(&[
                "task",
                "status",
                "not-a-handle",
                "--actor",
                "agent:x",
                "--capability",
                "cap_x",
            ]),
            &mut transport,
            false,
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn context_expand_requires_the_three_named_flags() {
        let mut transport = Deaf;
        let error = run(
            &args(&[
                "context",
                "expand",
                "--actor",
                "agent:x",
                "--capability",
                "cap_x",
            ]),
            &mut transport,
            false,
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn task_resume_without_states_is_a_usage_error_and_never_reaches_the_wire() {
        let mut transport = Deaf;
        let error = run(
            &args(&[
                "task",
                "resume",
                "cont_abc123",
                "--actor",
                "agent:x",
                "--capability",
                "cap_x",
            ]),
            &mut transport,
            false,
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 1);
        assert!(matches!(error, CliError::Usage(_)));
    }

    #[test]
    fn an_unknown_expansion_relation_is_a_usage_error() {
        let mut transport = Deaf;
        let error = run(
            &args(&[
                "context",
                "expand",
                "--context",
                "ctx_abc123",
                "--anchor",
                "n1",
                "--relation",
                "not-a-relation",
                "--actor",
                "agent:x",
                "--capability",
                "cap_x",
            ]),
            &mut transport,
            false,
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn the_inline_flag_takes_no_value_and_does_not_swallow_the_handle() {
        // `--inline` before the handle: if it consumed the next word, `ev_abc123` would be
        // its value and the command would have no positional handle at all. Reaching the
        // (deaf) wire — exit `2`, a connection failure — is what proves the handle parsed.
        let mut transport = Deaf;
        let error = run(
            &args(&[
                "evidence",
                "show",
                "--inline",
                "ev_abc123",
                "--actor",
                "agent:x",
                "--capability",
                "cap_x",
            ]),
            &mut transport,
            false,
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 2);
        assert!(matches!(error, CliError::Connection(_)));
    }

    #[test]
    fn every_noun_this_build_recognizes_is_dispatched_rather_than_rejected() {
        // A noun in `NOUNS` that `run` did not have an arm for would fail here as an
        // "unknown command" usage error carrying the same list — which is exactly the drift
        // the constant exists to prevent.
        for noun in NOUNS {
            let mut transport = Deaf;
            let error = run(
                &args(&[noun, "--actor", "agent:x", "--capability", "cap_x"]),
                &mut transport,
                false,
            )
            .unwrap_err();
            let CliError::Usage(detail) = &error else {
                panic!("a noun with no verb is a usage error: {error:?}");
            };
            assert!(
                detail.starts_with("usage: continuum "),
                "{noun:?} reached its own dispatcher's usage line: {detail}"
            );
        }
    }

    #[test]
    fn an_unknown_verb_in_a_recognized_noun_group_names_the_verbs_that_exist() {
        for (noun, expected) in [
            ("debug", "\"open\" or \"state\""),
            ("repair", "\"begin\" or \"review\""),
            ("evidence", "\"show\""),
        ] {
            let mut transport = Deaf;
            let error = run(
                &args(&[
                    noun,
                    "promote",
                    "--actor",
                    "agent:x",
                    "--capability",
                    "cap_x",
                ]),
                &mut transport,
                false,
            )
            .unwrap_err();
            let CliError::Usage(detail) = &error else {
                panic!("an unknown verb is a usage error: {error:?}");
            };
            assert!(detail.contains(expected), "{noun}: {detail}");
        }
    }

    #[test]
    fn json_flag_is_a_hidden_alias_for_format_json() {
        // `--format` and `--json` disagree here on purpose; `--json` still wins because it
        // is checked first — this only proves the flag is *read*, not that it outranks an
        // explicit `--format`, which `format::resolve` (unit-tested separately) already
        // pins wins for whichever the caller passes as `explicit`.
        let scan = Scan::of(&args(&["--json"])).expect("a flag-only line parses");
        assert!(scan.json_flag);
    }
}
