//! Every derived-handle decision is recorded for replay (cr-3lrkq3, bn-3hk4v).
//!
//! A replay returns a recorded outcome without running the handler, so any authorization
//! the handler decided on a handle the request did not name must be re-decided at replay.
//! The daemon does this with one mechanism: `Call::admits` (and `Call::derived`, which
//! calls it) records every derived handle it decides, the dispatcher files the set with the
//! replay record, and step 7 of `Daemon::dispatch` re-decides each one against the
//! presenting grant. A handler that decided a derived handle by another route would escape
//! the replay check. This audit reads the daemon's sources and refuses such a route.
//!
//! Two routes exist and are checked:
//!
//! - `admits_derived(` called directly. Only `admission.rs` (its definition), `family.rs`
//!   (`Call::admits`) and `mod.rs` (the replay re-check) may call it.
//! - `InstanceScope::of(`, the grant's instance scope used as a filter. Each use is listed
//!   with why it cannot bypass a replay: a read-only operation never reaches the ledger, and
//!   `whiteboard.compile`, the one `@mutation`, records each node a reference resolves
//!   through by `call.admits`.
//!
//! The runtime half is `c018_model_binding_system_path.rs`, whose narrow-grant replay test
//! fails without the mechanism.

use std::path::{Path, PathBuf};

fn sources(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("the source tree is readable") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Source lines that are code, not comments.
fn code_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter(|line| {
        let trimmed = line.trim_start();
        !(trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("//!"))
    })
}

fn users(pattern: &str) -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    sources(&root)
        .into_iter()
        .filter(|path| {
            let text = std::fs::read_to_string(path).expect("a source file is readable");
            code_lines(&text).any(|line| line.contains(pattern))
        })
        .map(|path| {
            path.strip_prefix(&root)
                .expect("under src")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

#[test]
fn only_call_admits_decides_a_derived_handle() {
    assert_eq!(
        users("admits_derived("),
        vec![
            "daemon/admission.rs".to_owned(),
            "daemon/family.rs".to_owned(),
            "daemon/mod.rs".to_owned(),
        ],
        "a handler that calls `admits_derived` directly escapes the replay record; use \
         `call.admits` or `call.derived`"
    );
}

#[test]
fn no_other_grant_decision_is_named_outside_admission() {
    // The other routes a handler could decide a handle by without `call.admits`: the
    // public admission helpers, the grant's scope fields read directly, and a renamed
    // import of an admission item. None may appear in a handler module.
    for pattern in [
        "admits_instance(",
        "is_unscoped(",
        "covers(",
        "grant.snapshots",
        "grant.intents",
        "grant.artifact_classes",
        "grant.instances",
        "admission::admits_derived as",
        "InstanceScope as",
    ] {
        let found: Vec<String> = users(pattern)
            .into_iter()
            .filter(|file| file != "daemon/admission.rs")
            .collect();
        let allowed: &[&str] = match pattern {
            // The replay decision itself.
            "covers(" => &["daemon/mod.rs"],
            _ => &[],
        };
        assert_eq!(found, allowed, "{pattern} used outside admission");
    }
    // `admits_event` is the subscription-delivery predicate: its module and the transport
    // that delivers subscriptions, neither of them a replayed call.
    assert_eq!(
        users("admits_event("),
        vec![
            "daemon/evidence.rs".to_owned(),
            "transport/mod.rs".to_owned()
        ],
        "admits_event is used only by @readonly subscription delivery"
    );
}

#[test]
fn every_instance_scope_filter_is_accounted_for() {
    // (file, why a replay cannot bypass it)
    let accounted = [
        (
            "daemon/admission.rs",
            "the definition, and admission's own T2 decision",
        ),
        (
            "daemon/evidence.rs",
            "evidence.query and evidence.subscribe: @readonly, never in the ledger",
        ),
        (
            "daemon/whiteboard.rs",
            "whiteboard.compile records each resolving and supporting node by call.admits",
        ),
        (
            "transport/mod.rs",
            "subscription delivery, not a replayed call",
        ),
    ];
    let expected: Vec<String> = accounted
        .iter()
        .map(|(file, _)| (*file).to_owned())
        .collect();
    assert_eq!(users("InstanceScope::of("), expected);

    let whiteboard = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/daemon/whiteboard.rs"),
    )
    .expect("readable");
    assert!(
        code_lines(&whiteboard).any(|line| line.contains("call.admits(")),
        "whiteboard.compile records the nodes its references resolve through"
    );
}

#[test]
fn the_replay_record_carries_the_consulted_handles_and_the_replay_rechecks_them() {
    let dispatch =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/daemon/mod.rs"))
            .expect("readable");
    let code: Vec<&str> = code_lines(&dispatch).collect();
    assert!(
        code.iter()
            .any(|line| line.contains("derived: consulted.take()")),
        "the replay record files every derived handle the call decided"
    );
    assert!(
        code.iter().any(|line| line.contains(".derived")
            && dispatch.contains("admits_derived(&grant, handle.as_derived())")),
        "a replay re-decides every recorded derived handle against the presenting grant"
    );
}
