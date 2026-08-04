//! `continuum` — the PR-13 human CLI binary.
//!
//! This build recognizes the command groups bn-3tz60 and bn-1g7e4 own: `context expand`,
//! `task status|resume|cancel`, `debug open|state`, `repair begin|review`, and
//! `evidence show`. The remaining verbs the plan names (`snapshot`, `check`, `explain`)
//! belong to a sibling bone (`continuum_cli`'s crate doc names which) and are not recognized
//! here.
//!
//! It connects over [`continuum_cli::wire::NullTransport`] — see the library crate's root
//! doc for why: `continuumd` ships no socket or process transport yet, so there is no real
//! deployment for this binary to dial. A wire call therefore always answers
//! `LinkError::NoTransportConfigured`, honestly, with exit code `2` (a connection failure,
//! per the conventions doc's exit-code taxonomy). Argument parsing and usage errors are
//! fully real and exit `1` on their own terms, without needing a transport at all.

use std::io::IsTerminal;
use std::process::ExitCode;

use continuum_cli::cli;
use continuum_cli::wire::NullTransport;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let stdout_is_tty = std::io::stdout().is_terminal();
    let mut transport = NullTransport;

    match cli::run(&args, &mut transport, stdout_is_tty) {
        Ok(rendered) => {
            print!("{}", rendered.text);
            #[allow(clippy::cast_sign_loss)]
            ExitCode::from(rendered.exit_code as u8)
        }
        Err(error) => {
            eprintln!("{error}");
            #[allow(clippy::cast_sign_loss)]
            ExitCode::from(error.exit_code() as u8)
        }
    }
}
