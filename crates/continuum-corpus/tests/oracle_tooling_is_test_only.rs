//! Oracle tooling stays test-only (bn-34mw; ADR-0029 "foreign oracle tooling does not
//! ship", applied to the differential harness).
//!
//! Proved from `cargo metadata` dependency kinds, not from a text scan of manifests:
//!
//! 1. every workspace edge onto `continuum-corpus` is a `dev` edge;
//! 2. no workspace package with a `bin` target reaches `continuum-corpus` through
//!    normal or build edges (any target, optional or not);
//! 3. `continuum-corpus` has no `bin` target of its own.
//!
//! The same check runs over two injected graphs, a direct and a transitive normal edge
//! from `continuum-cli`, and must reject both, so it cannot pass vacuously.

mod support {
    pub mod json;
}

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;

use support::json::{self, Json};

const HARNESS: &str = "continuum-corpus";

/// One workspace package: its name, whether it builds a binary, and its dependencies
/// as `(package name, kind)` with kind `normal`, `build` or `dev`.
#[derive(Debug, Clone)]
struct Package {
    name: String,
    has_bin: bool,
    dependencies: Vec<(String, String)>,
}

fn packages(metadata: &Json) -> Vec<Package> {
    let members: BTreeSet<&str> = metadata
        .get("workspace_members")
        .and_then(Json::as_array)
        .expect("workspace_members")
        .iter()
        .filter_map(Json::as_str)
        .collect();
    metadata
        .get("packages")
        .and_then(Json::as_array)
        .expect("packages")
        .iter()
        .filter(|package| {
            package
                .get("id")
                .and_then(Json::as_str)
                .is_some_and(|id| members.contains(id))
        })
        .map(|package| Package {
            name: package
                .get("name")
                .and_then(Json::as_str)
                .expect("name")
                .to_owned(),
            has_bin: package
                .get("targets")
                .and_then(Json::as_array)
                .expect("targets")
                .iter()
                .any(|target| {
                    target
                        .get("kind")
                        .and_then(Json::as_array)
                        .is_some_and(|kinds| kinds.iter().any(|k| k.as_str() == Some("bin")))
                }),
            dependencies: package
                .get("dependencies")
                .and_then(Json::as_array)
                .expect("dependencies")
                .iter()
                .map(|dependency| {
                    let name = dependency
                        .get("name")
                        .and_then(Json::as_str)
                        .expect("dep name");
                    let kind = match dependency.get("kind") {
                        Some(Json::String(kind)) => kind.clone(),
                        Some(Json::Null) | None => "normal".to_owned(),
                        Some(other) => panic!("unexpected dependency kind {other:?}"),
                    };
                    (name.to_owned(), kind)
                })
                .collect(),
        })
        .collect()
}

/// Every violation of rules 1–3, as text.
fn violations(packages: &[Package]) -> Vec<String> {
    let mut out = Vec::new();
    let names: BTreeSet<&str> = packages.iter().map(|p| p.name.as_str()).collect();
    assert!(
        names.contains(HARNESS),
        "the harness crate is a workspace member"
    );
    let shipped: BTreeMap<&str, Vec<&str>> = packages
        .iter()
        .map(|p| {
            (
                p.name.as_str(),
                p.dependencies
                    .iter()
                    .filter(|(name, kind)| kind != "dev" && names.contains(name.as_str()))
                    .map(|(name, _)| name.as_str())
                    .collect(),
            )
        })
        .collect();
    for package in packages {
        for (name, kind) in &package.dependencies {
            if name == HARNESS && kind != "dev" {
                out.push(format!(
                    "{} depends on {HARNESS} as a {kind} dependency",
                    package.name
                ));
            }
        }
        if package.name == HARNESS && package.has_bin {
            out.push(format!("{HARNESS} has a bin target"));
        }
        if !package.has_bin {
            continue;
        }
        // Normal/build closure from a binary package, with a witness path.
        let mut seen: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut stack: Vec<Vec<&str>> = vec![vec![package.name.as_str()]];
        while let Some(path) = stack.pop() {
            let node = *path.last().expect("non-empty path");
            for next in shipped.get(node).into_iter().flatten() {
                if seen.contains_key(next) {
                    continue;
                }
                let mut longer = path.clone();
                longer.push(next);
                seen.insert(next, longer.clone());
                stack.push(longer);
            }
        }
        if let Some(path) = seen.get(HARNESS) {
            out.push(format!(
                "release binary package reaches the harness: {}",
                path.join(" -> ")
            ));
        }
    }
    out
}

fn workspace_metadata() -> Json {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ])
        .current_dir(root)
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    json::parse(&String::from_utf8(output.stdout).expect("utf-8")).expect("cargo metadata is JSON")
}

#[test]
fn no_release_binary_links_the_differential_harness() {
    let packages = packages(&workspace_metadata());
    let binaries: Vec<&str> = packages
        .iter()
        .filter(|p| p.has_bin)
        .map(|p| p.name.as_str())
        .collect();
    // Anti-vacuity: the release binary exists and is checked. `continuumd` is a
    // library the `continuum` binary links, so it is covered by the closure walk.
    assert!(
        binaries.contains(&"continuum-cli"),
        "continuum-cli has a bin target: {binaries:?}"
    );
    // Anti-vacuity: the harness has consumers only through dev edges, and at least
    // one engine is linked by it only as dev (the adapters' side).
    let harness = packages
        .iter()
        .find(|p| p.name == HARNESS)
        .expect("harness");
    assert!(
        harness
            .dependencies
            .iter()
            .any(|(name, kind)| name == "continuum-engine-reference" && kind == "dev")
    );
    assert!(
        !harness
            .dependencies
            .iter()
            .any(|(name, kind)| name.starts_with("continuum-engine-") && kind != "dev")
    );
    assert!(
        !harness
            .dependencies
            .iter()
            .any(|(name, kind)| name.starts_with("continuum-kernel-") && kind != "dev")
    );
    assert_eq!(violations(&packages), Vec::<String>::new());
}

#[test]
fn an_injected_release_edge_onto_the_harness_is_rejected() {
    let real = packages(&workspace_metadata());

    let mut direct = real.clone();
    direct
        .iter_mut()
        .find(|p| p.name == "continuum-cli")
        .expect("cli")
        .dependencies
        .push((HARNESS.to_owned(), "normal".to_owned()));
    let found = violations(&direct);
    assert!(
        found
            .iter()
            .any(|v| v.contains("continuum-cli depends on continuum-corpus as a normal")),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|v| v.contains("continuum-cli -> continuum-corpus")),
        "{found:?}"
    );

    // Transitive: a library the daemon links picks the harness up as a build edge.
    let mut transitive = real.clone();
    transitive
        .iter_mut()
        .find(|p| p.name == "continuum-value")
        .expect("value")
        .dependencies
        .push((HARNESS.to_owned(), "build".to_owned()));
    let found = violations(&transitive);
    assert!(
        found.iter().any(|v| v.starts_with(
            "release binary package reaches the harness: continuum-cli -> continuumd"
        ) && v.ends_with("-> continuum-corpus")),
        "{found:?}"
    );

    // A dev edge is admitted: that is how tests link it.
    let mut dev = real;
    dev.iter_mut()
        .find(|p| p.name == "continuum-cli")
        .expect("cli")
        .dependencies
        .push((HARNESS.to_owned(), "dev".to_owned()));
    assert_eq!(violations(&dev), Vec::<String>::new());
}
