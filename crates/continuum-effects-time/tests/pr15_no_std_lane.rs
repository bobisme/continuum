//! The compiler lane behind "the time pack links no `std`" (bn-1oj6). It is the
//! network pack's lane (bn-3ohe, cr-1dl1d7) applied unchanged to this crate: the
//! crates share no test code, because a dev-dependency between them would be a new
//! dependency edge.
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

// The shared token lexer and structure rules (cr-35ujnx): one Rust file for the
// INV-015 audit and the three pack lanes, the twin of `tools/governance/rust_lexer.py`.
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/governance/rust_lexer.rs"
));

/// Why the crate's structure lets some build escape `no_std`, or `Ok`.
fn structure(sources: &[(String, String)]) -> Result<(), String> {
    rust_lexer::structure(
        sources
            .iter()
            .map(|(name, text)| (name.as_str(), text.as_str())),
    )
}

/// Why Cargo's resolved view of the package at `root` lets a build escape its `src/`
/// (cr-35ujnx round 5), or `Ok`. Cargo, not a scan of the manifest text, decides what
/// a quoted, dotted or inline `build` or `links` key means, and finds a `build.rs`
/// with no key at all. A manifest Cargo refuses is refused.
fn cargo_problem(root: &Path, package: &str, locked: bool) -> Result<(), String> {
    let meta = cargo_view::metadata(&root.join("Cargo.toml"), locked)?;
    cargo_view::pack_problem(&meta, package, root)
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
    scratch_with(name, sources, &lane_manifest(), &[])
}

/// The lane's scratch package name.
const LANE_PACKAGE: &str = "time-pack-lane";

/// The scratch copy's own manifest: its own workspace root, the workspace's edition and
/// `unsafe_code = "forbid"`.
fn lane_manifest() -> String {
    let workspace = std::fs::read_to_string(manifest_dir().join("../../Cargo.toml")).unwrap();
    let edition = workspace
        .lines()
        .find_map(|line| line.trim().strip_prefix("edition = "))
        .expect("the workspace declares an edition");
    assert!(
        workspace.contains("unsafe_code = \"forbid\""),
        "the workspace no longer sets unsafe_code to forbid"
    );
    format!(
        "[package]\nname = \"{LANE_PACKAGE}\"\nversion = \"0.0.0\"\nedition = {edition}\n\
         publish = false\n\n[lints.rust]\nunsafe_code = \"forbid\"\n\n[workspace]\n"
    )
}

/// A standalone copy of the crate with the given manifest and extra root files, under a
/// directory unique to this process.
fn scratch_with(
    name: &str,
    sources: &[(String, String)],
    manifest: &str,
    extra: &[(&str, &str)],
) -> PathBuf {
    let root = manifest_dir()
        .join("../../target/pr15-time-no-std-lane")
        .join(format!("{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    for (file, text) in sources {
        std::fs::write(root.join("src").join(file), text).unwrap();
    }
    std::fs::write(root.join("Cargo.toml"), manifest).unwrap();
    for (file, text) in extra {
        std::fs::write(root.join(file), text).unwrap();
    }
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

/// The dep-info rustc wrote for the scratch crate's library in one profile: every
/// file and every environment variable the compiler read to build it.
fn dep_info(package: &Path, release: bool) -> String {
    let deps = package
        .join("target")
        .join(if release { "release" } else { "debug" })
        .join("deps");
    let mut found: Vec<String> = std::fs::read_dir(&deps)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "d"))
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect();
    assert_eq!(found.len(), 1, "one dep-info file in {}", deps.display());
    found.remove(0)
}

/// The compiler's own witness that a build read nothing from its environment
/// (cr-35ujnx round 4): its dep-info names no `# env-dep:` variable, and every file it
/// read lies in the copy's `src/`.
fn ambient_inputs(package: &Path, release: bool) -> Vec<String> {
    let src = package.join("src").canonicalize().unwrap();
    let mut out = Vec::new();
    for line in dep_info(package, release).lines() {
        if let Some(var) = line.strip_prefix("# env-dep:") {
            out.push(format!("env {var}"));
        } else if let Some((_, deps)) = line.split_once(": ") {
            for dep in deps.split_whitespace() {
                let path = package.join(dep);
                let path = path.canonicalize().unwrap_or(path);
                if !path.starts_with(&src) {
                    out.push(format!("file {dep}"));
                }
            }
        }
    }
    out
}

#[test]
fn the_compiler_refuses_a_host_facility_in_the_time_pack() {
    let sources = real_sources();
    assert_eq!(structure(&sources), Ok(()));
    // Cargo's view of the real pack, in its real workspace context.
    assert_eq!(
        cargo_problem(manifest_dir(), env!("CARGO_PKG_NAME"), true),
        Ok(()),
        "cargo metadata shows the real pack escaping its src/"
    );
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
    // cr-35ujnx: `#![no_std]` does not forbid an explicit `extern crate std`, and an
    // alias split across lines reaches `std::fs` under another name. The compiler
    // accepts this copy; the structure rule is what refuses it, and the lane shows
    // both halves.
    let aliased_sources = with_aliased_std(&sources);
    assert!(
        structure(&aliased_sources).is_err(),
        "the structure rules accepted a split, aliased `extern crate std`"
    );
    let aliased = scratch_copy("aliased", &aliased_sources);
    // cr-35ujnx round 4: a pack must read nothing from its build environment. A copy
    // that reads an environment variable compiles; the structure rule refuses it, and
    // the compiler's dep-info names the variable, while the real pack's names none.
    let ambient_sources: Vec<(String, String)> = sources
        .iter()
        .map(|(name, text)| {
            let text = if name == "lib.rs" {
                format!(
                    "{text}\n/// Planted by the lane.\npub const LANE_MODE: Option<&str> = \
                     option_env!(\"CONTINUUM_LANE_MODE\");\n"
                )
            } else {
                text.clone()
            };
            (name.clone(), text)
        })
        .collect();
    assert!(
        structure(&ambient_sources).is_err(),
        "the structure rules accepted an ambient `option_env!`"
    );
    let ambient = scratch_copy("ambient", &ambient_sources);
    for release in [false, true] {
        let clean = cargo_check(&control, release);
        assert!(
            clean.status.success(),
            "the unchanged copy must compile (release: {release}): {}",
            String::from_utf8_lossy(&clean.stderr)
        );
        assert_eq!(
            cargo_problem(&control, LANE_PACKAGE, true),
            Ok(()),
            "cargo metadata shows the control copy escaping its src/"
        );
        assert!(
            !dep_info(&control, release).contains("env-dep:OUT_DIR"),
            "a build script's OUT_DIR reached the pack"
        );
        assert_eq!(
            ambient_inputs(&control, release),
            Vec::<String>::new(),
            "rustc read an ambient input building the pack (release: {release})"
        );
        let through_env = cargo_check(&ambient, release);
        assert!(
            through_env.status.success(),
            "the env-reading copy must compile (release: {release}): {}",
            String::from_utf8_lossy(&through_env.stderr)
        );
        assert!(
            ambient_inputs(&ambient, release)
                .iter()
                .any(|input| input.starts_with("env CONTINUUM_LANE_MODE")),
            "rustc's dep-info does not name the variable the copy read, so an empty \
             env-dep set would prove nothing"
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
        let through_alias = cargo_check(&aliased, release);
        assert!(
            through_alias.status.success(),
            "the aliased copy must compile, or the structure rule is not what refuses it \
             (release: {release}): {}",
            String::from_utf8_lossy(&through_alias.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(control);
    let _ = std::fs::remove_dir_all(planted);
    let _ = std::fs::remove_dir_all(aliased);
    let _ = std::fs::remove_dir_all(ambient);
}

/// The crate's sources with `extern crate std as s;` split across two lines after
/// `extern crate alloc;`, and a function that reaches the filesystem through the alias.
fn with_aliased_std(sources: &[(String, String)]) -> Vec<(String, String)> {
    sources
        .iter()
        .map(|(name, text)| {
            let text = if name == "lib.rs" {
                format!(
                    "{}\n/// Planted by the lane.\npub fn lane_alias() -> bool {{\n    \
                     s::fs::metadata(\"/\").is_ok()\n}}\n",
                    text.replacen(
                        "extern crate alloc;",
                        "extern crate alloc;\nextern\ncrate std as s;",
                        1
                    )
                )
            } else {
                text.clone()
            };
            (name.clone(), text)
        })
        .collect()
}

/// The structure rules refuse each known way around them.
#[test]
fn the_structure_rules_refuse_each_way_around_no_std() {
    let sources = real_sources();
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
        (
            "split, aliased extern crate std",
            with_aliased_std(&sources),
        ),
        (
            "extern crate core",
            edit_lib(&|t| {
                t.replacen(
                    "extern crate alloc;",
                    "extern crate alloc;\nextern crate core as c;",
                    1,
                )
            }),
        ),
        (
            "aliased alloc",
            edit_lib(&|t| t.replacen("extern crate alloc;", "extern crate alloc as a;", 1)),
        ),
        (
            "a string that opens a comment hides the extern",
            edit_lib(&|t| format!("{t}\npub const LANE: &str = \"//\"; extern crate std as s;\n")),
        ),
    ];
    for (name, mutant) in lib_mutants {
        assert!(structure(&mutant).is_err(), "{name} passed");
    }
    // Manifest forms, judged by Cargo's resolved view (cr-35ujnx round 5): quoted,
    // dotted, inherited and inline-table `build` and `links` keys, a `build.rs` that
    // Cargo finds with no key at all, a feature and dependencies. Each is refused,
    // either because Cargo resolves it to what the rule forbids or because Cargo
    // refuses the manifest.
    let base = lane_manifest();
    let script = "fn main() {}\n";
    let inline = format!(
        "package = {{ name = \"{LANE_PACKAGE}\", version = \"0.0.0\", edition = \"2024\", \
         publish = false, build = \"x.rs\" }}\n\n[workspace]\n"
    );
    type Variant<'a> = (&'a str, String, Vec<(&'a str, &'a str)>);
    let variants: Vec<Variant> = vec![
        (
            "quoted build key",
            base.replacen(
                "publish = false",
                "publish = false\n\"build\" = \"x.rs\"",
                1,
            ),
            vec![("x.rs", script)],
        ),
        (
            "single-quoted links key with a build script",
            base.replacen("publish = false", "publish = false\n'links' = \"z\"", 1),
            vec![("build.rs", script)],
        ),
        (
            "dotted package.\"links\" key",
            format!("package.\"links\" = \"z\"\n{base}"),
            vec![],
        ),
        (
            "inherited build key",
            base.replacen(
                "publish = false",
                "publish = false\nbuild.workspace = true",
                1,
            ),
            vec![("build.rs", script)],
        ),
        (
            "inline-table package with a build key",
            inline,
            vec![("x.rs", script)],
        ),
        (
            "build.rs found with no key",
            base.clone(),
            vec![("build.rs", script)],
        ),
        (
            "std feature",
            format!("{base}\n[features]\nstd = []\n"),
            vec![],
        ),
        (
            "target dependency",
            format!("{base}\n[target.'cfg(windows)'.dependencies]\nx = \"1\"\n"),
            vec![],
        ),
        (
            "dependency table",
            format!("{base}\n[dependencies.foo]\npath = \"../foo\"\n"),
            vec![],
        ),
    ];
    let clean = scratch_with("manifest-clean", &sources, &base, &[]);
    assert_eq!(cargo_problem(&clean, LANE_PACKAGE, false), Ok(()));
    let _ = std::fs::remove_dir_all(clean);
    for (at, (name, manifest, extra)) in variants.into_iter().enumerate() {
        let root = scratch_with(&format!("manifest-{at}"), &sources, &manifest, &extra);
        assert!(
            cargo_problem(&root, LANE_PACKAGE, false).is_err(),
            "{name} passed Cargo's view"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
