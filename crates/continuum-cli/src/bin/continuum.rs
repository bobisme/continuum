//! `continuum` — the PR-13 human CLI binary.
//!
//! This build recognizes every command group the three PR-13 command bones own:
//! `snapshot create|fork|seal`, `check start|result|await`, `explain compile` (bn-3rqvm);
//! `context expand`, `task status|resume|cancel` (bn-3tz60); and `debug open|state`,
//! `repair begin|review`, `evidence show` (bn-1g7e4). What they all print is
//! [`continuum_cli::contract`] (bn-ybh1z).
//!
//! # Exit codes
//!
//! The five classes of [`continuum_cli::contract::Exit`], and that module's doc is where
//! they are documented:
//!
//! | Code | Meaning |
//! |---|---|
//! | `0` | The daemon answered, and no verdict it carried says no. |
//! | `1` | No answer: the request was typedly refused, or the command line did not parse. |
//! | `2` | No answer: the connection failed below the protocol. |
//! | `3` | An answer, and it is **no** — a decided negative verdict. |
//! | `4` | An answer, and it is INV-008's typed **unknown**. |
//!
//! # What this binary can reach today
//!
//! It connects over [`continuum_cli::wire::NullTransport`] — see the library crate's root
//! doc for why: `continuumd` ships no socket or process transport yet, so there is no real
//! deployment for this binary to dial. A wire call therefore always answers
//! `LinkError::NoTransportConfigured`, honestly, with exit code `2`. Argument parsing and
//! usage errors are fully real and exit `1` on their own terms, without needing a transport
//! at all.

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
