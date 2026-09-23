//! No deterministic path can reach the operating-system entropy source (bn-1hape, INV-005,
//! ADR-0003).
//!
//! `OsEntropy` is the one production `KeyEntropy`, and it lives in `continuum-security`, a
//! boundary crate. This file reads the real, committed manifests and source tree — not a
//! model of them — and checks three facts:
//!
//! 1. no semantic-core crate (`tools/governance/check_code_policy.py`'s `SEMANTIC_CORE`,
//!    read live) reaches `continuum-security` through a normal or build dependency, at any
//!    depth, so no semantic-core code can name `OsEntropy`;
//! 2. the kernel's entropy device is named in exactly one source file,
//!    `continuum-security/src/entropy.rs`;
//! 3. `continuum-evidence`, which mints keys, names no entropy source at all: its only
//!    entropy is the `KeyEntropy` value a caller passes in.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the workspace root is two levels up")
        .to_path_buf()
}

fn crate_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(root().join("crates"))
        .expect("crates/")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.join("Cargo.toml").is_file())
        .collect();
    dirs.sort();
    dirs
}

/// Workspace-internal normal and build edges, read from each manifest's dependency tables
/// (dev-dependencies are not shipped and are excluded).
fn shipped_edges() -> BTreeMap<String, BTreeSet<String>> {
    let mut edges = BTreeMap::new();
    for dir in crate_dirs() {
        let name = dir
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        let text = fs::read_to_string(dir.join("Cargo.toml")).expect("manifest");
        let mut shipped = false;
        let mut deps = BTreeSet::new();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                shipped = (line.ends_with("dependencies]") && !line.contains("dev-dependencies"))
                    || line.starts_with("[dependencies.")
                    || line.starts_with("[build-dependencies.");
                if let Some(dep) = line
                    .strip_prefix("[dependencies.")
                    .or_else(|| line.strip_prefix("[build-dependencies."))
                {
                    deps.insert(dep.trim_end_matches(']').to_owned());
                }
                continue;
            }
            if !shipped {
                continue;
            }
            let key: String = line
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if key.starts_with("continuum") {
                deps.insert(key);
            }
        }
        edges.insert(name, deps);
    }
    edges
}

fn semantic_core() -> BTreeSet<String> {
    let text = fs::read_to_string(root().join("tools/governance/check_code_policy.py"))
        .expect("check_code_policy.py");
    let start = text
        .find("SEMANTIC_CORE = frozenset(")
        .expect("SEMANTIC_CORE is declared");
    let block = &text[start..];
    let end = block.find(')').expect("the frozenset closes");
    let names: BTreeSet<String> = block[..end]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    assert!(
        names.contains("continuum-evidence") && names.len() >= 20,
        "SEMANTIC_CORE drifted: {names:?}"
    );
    names
}

fn reaches(edges: &BTreeMap<String, BTreeSet<String>>, from: &str, target: &str) -> bool {
    let mut stack = vec![from.to_owned()];
    let mut seen = BTreeSet::new();
    while let Some(node) = stack.pop() {
        if node == target {
            return true;
        }
        if seen.insert(node.clone()) {
            stack.extend(edges.get(&node).into_iter().flatten().cloned());
        }
    }
    false
}

fn sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for dir in crate_dirs() {
        let mut stack = vec![dir.join("src")];
        while let Some(path) = stack.pop() {
            let Ok(entries) = fs::read_dir(&path) else {
                continue;
            };
            for entry in entries {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    let rel = path
                        .strip_prefix(root())
                        .expect("under root")
                        .to_string_lossy()
                        .into_owned();
                    out.push((rel, fs::read_to_string(&path).expect("source")));
                }
            }
        }
    }
    out.sort();
    out
}

#[test]
fn no_semantic_core_crate_can_reach_the_os_entropy_source() {
    let edges = shipped_edges();
    assert!(
        edges["continuum-security"].contains("continuum-evidence"),
        "the manifest reader sees the edge this bone added (non-vacuity)"
    );
    let core = semantic_core();
    let reaching: Vec<&String> = core
        .iter()
        .filter(|name| reaches(&edges, name, "continuum-security"))
        .collect();
    assert!(
        reaching.is_empty(),
        "semantic-core crates reach continuum-security: {reaching:?}"
    );
}

#[test]
fn the_entropy_device_is_named_in_exactly_one_source_file() {
    let naming: Vec<String> = sources()
        .into_iter()
        .filter(|(_, text)| text.contains("/dev/urandom") || text.contains("/dev/random"))
        .map(|(rel, _)| rel)
        .collect();
    assert_eq!(naming, ["crates/continuum-security/src/entropy.rs"]);
}

#[test]
fn continuum_evidence_names_no_entropy_source() {
    for (rel, text) in sources() {
        if rel.starts_with("crates/continuum-evidence/") {
            for forbidden in ["OsEntropy", "continuum_security", "/dev/"] {
                assert!(!text.contains(forbidden), "{rel} names {forbidden}");
            }
        }
    }
}
