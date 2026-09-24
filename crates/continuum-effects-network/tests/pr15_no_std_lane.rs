//! The compiler lane behind "the network pack links no `std`" (bn-3ohe, cr-1dl1d7).
//!
//! The crate is `#![no_std]` and links only `core` and `alloc`, and `unsafe_code` is
//! forbidden workspace-wide, so no FFI route exists. This lane shows the compiler
//! actually enforces that for the builds that exist. It copies the crate's `src/` into
//! a scratch package and runs `cargo check` in the dev and the release profile:
//!
//! - the unchanged copy must compile, or the lane proves nothing;
//! - a copy with a planted *use* of `std::net::UdpSocket` must fail with E0432 or
//!   E0433 naming the unresolved `std` crate, not with any other error.
//!
//! What the lane covers, and why that is every build: the check builds the host target
//! with no features, no build script, no dependencies and no `cfg`. The structure rules
//! below, which this lane, the INV-015 audit and the GOV-4-13 checker all enforce, make
//! those the only builds: no `cfg` or `cfg_attr` anywhere in `src/`, no macro
//! definitions, no `include!`, no `#[path]`, exactly one `extern crate`, which is
//! `alloc`, `#![no_std]` as the crate root's first item, and a manifest with only
//! `[package]` (no `build`, no `links`) and `[lints] workspace = true`. A std-less
//! target (`x86_64-unknown-none`) would add a second, independent witness, but it is
//! not in the pinned toolchain.

use std::path::{Path, PathBuf};
use std::process::Command;

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Code lines of a Rust source: `//` comments (and `//!`, `///`) dropped, blank lines
/// dropped, the rest trimmed. Block comments are refused by the structure rules.
fn code_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn words(line: &str) -> Vec<&str> {
    line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .collect()
}

/// Whether a line holds a raw identifier or raw string: an `r` that starts a token and
/// is followed by `#` or `"`.
fn raw_token(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.windows(2).enumerate().any(|(at, pair)| {
        pair[0] == b'r'
            && (pair[1] == b'#' || pair[1] == b'"')
            && (at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_'))
    })
}

/// Why the crate's structure lets some build escape `no_std`, or `Ok`.
fn structure(sources: &[(String, String)], manifest: &str) -> Result<(), String> {
    let lib = sources
        .iter()
        .find(|(name, _)| name == "lib.rs")
        .ok_or("no lib.rs")?;
    if code_lines(&lib.1).first().map(String::as_str) != Some("#![no_std]") {
        return Err("`#![no_std]` is not the crate root's first item".to_owned());
    }
    let mut externs = 0;
    for (name, text) in sources {
        if text.contains("/*") {
            return Err(format!("{name} has a block comment"));
        }
        for line in code_lines(text) {
            let w = words(&line);
            for banned in [
                "cfg",
                "cfg_attr",
                "macro_rules",
                "include",
                "include_str",
                "include_bytes",
                "path",
                "asm",
                "global_asm",
            ] {
                if w.contains(&banned) {
                    return Err(format!("{name} uses `{banned}`: {line}"));
                }
            }
            if raw_token(&line) {
                return Err(format!("{name} uses a raw identifier or string: {line}"));
            }
            if w.windows(2).any(|pair| pair == ["extern", "crate"]) {
                externs += 1;
                if name != "lib.rs" || line != "extern crate alloc;" {
                    return Err(format!(
                        "{name} has an extern crate other than alloc: {line}"
                    ));
                }
            }
        }
    }
    if externs != 1 {
        return Err(format!(
            "{externs} extern crate declarations, not exactly one"
        ));
    }
    let mut section = "";
    for line in manifest
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        if line.starts_with('[') {
            if line != "[package]" && line != "[lints]" {
                return Err(format!("the manifest has a `{line}` table"));
            }
            section = if line == "[package]" {
                "package"
            } else {
                "lints"
            };
            continue;
        }
        let key = line.split('=').next().unwrap_or("").trim();
        if section == "package" && (key == "build" || key == "links") {
            return Err(format!("the manifest sets `{key}`"));
        }
        if section == "lints" && line != "workspace = true" {
            return Err(format!("the manifest overrides lints: {line}"));
        }
        if section.is_empty() {
            return Err(format!("the manifest has a top-level key: {line}"));
        }
    }
    if !manifest.contains("[lints]\nworkspace = true") {
        return Err("the manifest does not inherit the workspace lints".to_owned());
    }
    Ok(())
}

fn real_sources() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = std::fs::read_dir(manifest_dir().join("src"))
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file(), "src/ must be flat");
            (
                entry.file_name().into_string().unwrap(),
                std::fs::read_to_string(entry.path()).unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

/// A standalone copy of the crate, its own workspace root, with the workspace's edition
/// and `unsafe_code = "forbid"`, under a directory unique to this process.
fn scratch_copy(name: &str, sources: &[(String, String)]) -> PathBuf {
    let root = manifest_dir()
        .join("../../target/pr15-no-std-lane")
        .join(format!("{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    for (file, text) in sources {
        std::fs::write(root.join("src").join(file), text).unwrap();
    }
    let workspace = std::fs::read_to_string(manifest_dir().join("../../Cargo.toml")).unwrap();
    let edition = workspace
        .lines()
        .find_map(|line| line.trim().strip_prefix("edition = "))
        .expect("the workspace declares an edition");
    assert!(
        workspace.contains("unsafe_code = \"forbid\""),
        "the workspace no longer sets unsafe_code to forbid"
    );
    std::fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"network-pack-lane\"\nversion = \"0.0.0\"\nedition = {edition}\n\
             publish = false\n\n[lints.rust]\nunsafe_code = \"forbid\"\n\n[workspace]\n"
        ),
    )
    .unwrap();
    root
}

fn cargo_check(package: &Path, release: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO"));
    command
        .arg("check")
        .arg("--offline")
        .arg("--quiet")
        .current_dir(package)
        .env("CARGO_TARGET_DIR", package.join("target"))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_BUILD_RUSTFLAGS");
    if release {
        command.arg("--release");
    }
    command.output().expect("cargo runs")
}

#[test]
fn the_compiler_refuses_a_host_facility_in_the_network_pack() {
    let sources = real_sources();
    let manifest = std::fs::read_to_string(manifest_dir().join("Cargo.toml")).unwrap();
    assert_eq!(structure(&sources, &manifest), Ok(()));
    assert!(
        !manifest_dir().join("build.rs").exists(),
        "the pack has a build script"
    );

    let control = scratch_copy("control", &sources);
    let planted_sources: Vec<(String, String)> = sources
        .iter()
        .map(|(name, text)| {
            let text = if name == "lib.rs" {
                format!(
                    "{text}\n/// Planted by the lane.\npub fn lane_plant() -> bool {{\n    \
                     std::net::UdpSocket::bind(\"127.0.0.1:0\").is_ok()\n}}\n"
                )
            } else {
                text.clone()
            };
            (name.clone(), text)
        })
        .collect();
    let planted = scratch_copy("planted", &planted_sources);
    for release in [false, true] {
        let clean = cargo_check(&control, release);
        assert!(
            clean.status.success(),
            "the unchanged copy must compile (release: {release}): {}",
            String::from_utf8_lossy(&clean.stderr)
        );
        let refused = cargo_check(&planted, release);
        let stderr = String::from_utf8_lossy(&refused.stderr);
        assert!(
            !refused.status.success(),
            "a no_std pack compiled with a planted std::net use (release: {release})"
        );
        assert!(
            (stderr.contains("E0433") || stderr.contains("E0432")) && stderr.contains("`std`"),
            "the failure must be the unresolved `std` crate, not another error: {stderr}"
        );
    }
    let _ = std::fs::remove_dir_all(control);
    let _ = std::fs::remove_dir_all(planted);
}

/// The structure rules refuse each known way around them.
#[test]
fn the_structure_rules_refuse_each_way_around_no_std() {
    let sources = real_sources();
    let manifest = std::fs::read_to_string(manifest_dir().join("Cargo.toml")).unwrap();
    let edit_lib = |f: &dyn Fn(&str) -> String| -> Vec<(String, String)> {
        sources
            .iter()
            .map(|(n, t)| (n.clone(), if n == "lib.rs" { f(t) } else { t.clone() }))
            .collect()
    };
    let lib_mutants: Vec<(&str, Vec<(String, String)>)> = vec![
        (
            "raw-string decoy",
            edit_lib(&|t| {
                t.replacen(
                    "\n#![no_std]\n",
                    "\nconst _D: &str = r\"\n#![no_std]\n\";\n",
                    1,
                )
            }),
        ),
        (
            "cfg_attr",
            edit_lib(&|t| t.replacen("\n#![no_std]\n", "\n#![cfg_attr(not(test), no_std)]\n", 1)),
        ),
        (
            "nested module",
            edit_lib(&|t| t.replacen("\n#![no_std]\n", "\nmod m {\n#![no_std]\n}\n", 1)),
        ),
        (
            "extern crate std",
            edit_lib(&|t| {
                t.replacen(
                    "extern crate alloc;",
                    "extern crate alloc;\nextern crate std;",
                    1,
                )
            }),
        ),
        (
            "raw identifier std",
            edit_lib(&|t| {
                t.replacen(
                    "extern crate alloc;",
                    "extern crate alloc;\nextern crate r#std;",
                    1,
                )
            }),
        ),
        (
            "cfg-gated extern",
            edit_lib(&|t| {
                t.replacen(
                    "extern crate alloc;",
                    "extern crate alloc;\n#[cfg(not(debug_assertions))]\nextern crate core as c;",
                    1,
                )
            }),
        ),
        (
            "macro indirection",
            edit_lib(&|t| {
                format!(
                    "{t}\nmacro_rules! k {{ ($c:ident) => {{ extern crate $c; }} }}\nk!(std);\n"
                )
            }),
        ),
        (
            "include",
            edit_lib(&|t| format!("{t}\ninclude!(\"../x.rs\");\n")),
        ),
        ("block comment", edit_lib(&|t| format!("{t}\n/* x */\n"))),
    ];
    for (name, mutant) in lib_mutants {
        assert!(structure(&mutant, &manifest).is_err(), "{name} passed");
    }
    for (name, bad) in [
        ("std feature", format!("{manifest}\n[features]\nstd = []\n")),
        (
            "target dependency",
            format!("{manifest}\n[target.'cfg(windows)'.dependencies]\nx = \"1\"\n"),
        ),
        (
            "dependency table",
            format!("{manifest}\n[dependencies.foo]\npath = \"../foo\"\n"),
        ),
        (
            "build script",
            manifest.replacen(
                "publish = false",
                "publish = false\nbuild = \"build.rs\"",
                1,
            ),
        ),
        (
            "lint override",
            manifest.replacen(
                "[lints]\nworkspace = true",
                "[lints]\nworkspace = true\nrust.unsafe_code = \"allow\"",
                1,
            ),
        ),
    ] {
        assert!(structure(&sources, &bad).is_err(), "{name} passed");
    }
}
