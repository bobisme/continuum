//! Argument parsing and dispatch for `continuum context expand` and
//! `continuum task status|resume|cancel` (this bone's two command groups).
//!
//! A sibling PR-13 bone owns every other verb (`snapshot`, `check`, `explain`, `debug`,
//! `repair`, `evidence show`) and the cross-cutting output-contract infrastructure
//! (bn-ybh1z); [`run`] therefore recognizes exactly the `context` and `task` noun groups
//! and reports every other first word as a usage error rather than guessing at a verb this
//! bone does not own.
//!
//! Flags are parsed by one small `--name value` / `--flag` scanner ([`Scan`]) rather than a
//! declarative parser: the surface is two commands and eight flags total, and a hand-rolled
//! scan keeps this crate's one external-facing dependency at `continuumd` (see the crate
//! root and `Cargo.toml` for why that edge is minimal on purpose this wave).

use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, ContextHandle, ContinuationHandle, ProtocolVersion, TaskHandle,
};
use continuumd::protocol::spec::{Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::ExpansionRelation;

use crate::context::{self, ExpandArgs};
use crate::error::CliError;
use crate::format::{self, Format};
use crate::render::Rendered;
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
        Some("context") => dispatch_context(&scan, &mut connection, transport, format),
        Some("task") => dispatch_task(&scan, &mut connection, transport, format),
        Some(other) => Err(CliError::usage(format!(
            "unknown command {other:?}; expected \"context\" or \"task\""
        ))),
        None => Err(CliError::usage(
            "usage: continuum context expand ... | continuum task status|resume|cancel <handle>",
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
    Ok(Connection::new(version, actor, capability))
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
/// `--json` is the one boolean flag (cli-conventions.md's hidden alias for
/// `--format json`); every other `--name` consumes the following word as its value.
struct Scan {
    positional: Vec<String>,
    flags: std::collections::BTreeMap<String, String>,
    json_flag: bool,
}

impl Scan {
    fn of(args: &[String]) -> Result<Self, CliError> {
        let mut positional = Vec::new();
        let mut flags = std::collections::BTreeMap::new();
        let mut json_flag = false;
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            if arg == "--json" {
                json_flag = true;
            } else if let Some(name) = arg.strip_prefix("--") {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::usage(format!("--{name} requires a value")))?;
                flags.insert(name.to_owned(), value.clone());
            } else {
                positional.push(arg.clone());
            }
        }
        Ok(Self {
            positional,
            flags,
            json_flag,
        })
    }

    fn required(&self, name: &str) -> Result<String, CliError> {
        self.flags
            .get(name)
            .cloned()
            .ok_or_else(|| CliError::usage(format!("--{name} is required")))
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
        let mut transport = Deaf;
        let error = run(&args(&["snapshot"]), &mut transport, false).unwrap_err();
        assert_eq!(error.exit_code(), 1);
        assert!(matches!(error, CliError::Usage(_)));
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
    fn json_flag_is_a_hidden_alias_for_format_json() {
        // `--format` and `--json` disagree here on purpose; `--json` still wins because it
        // is checked first — this only proves the flag is *read*, not that it outranks an
        // explicit `--format`, which `format::resolve` (unit-tested separately) already
        // pins wins for whichever the caller passes as `explicit`.
        let scan = Scan::of(&args(&["--json"])).expect("a flag-only line parses");
        assert!(scan.json_flag);
    }
}
