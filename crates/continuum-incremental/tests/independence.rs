//! INV-010: the clean side of the Incremental Parity Audit shares no decision logic
//! with the engine (bn-31vf).
//!
//! A source-level pin, the same kind the kernel's serialization-boundary checks use:
//! `src/audit.rs` may read the engine's records (`Revision`, `Record`, `Outcome`) and
//! its configured `Limits`, and nothing else from `crate::engine`. It never names the
//! engine, its decision functions, the key, the registry's edge specifications, the
//! quarantine, or the lanes.

const AUDIT: &str = include_str!("../src/audit.rs");

fn code_lines() -> impl Iterator<Item = &'static str> {
    AUDIT
        .lines()
        .map(str::trim_start)
        .filter(|line| !line.starts_with("//"))
}

#[test]
fn the_audit_imports_only_the_engines_read_only_records() {
    // Any path into the engine module, in any import form, is counted.
    let imports: Vec<&str> = code_lines()
        .filter(|line| {
            line.contains("engine::") || line.contains("engine,") || line.contains("engine}")
        })
        .collect();
    assert_eq!(
        imports,
        ["use crate::engine::{Limits, Memo, Outcome, Record, Revision};"]
    );
}

#[test]
fn the_audit_names_no_engine_decision_logic() {
    let forbidden = [
        "Engine",
        "decide",
        "admit",
        "demand",
        "QueryKey",
        "EdgeSpec",
        "Keyed",
        "heuristic_normalize",
        "experimental_parse",
        "validated_safety",
        "admits",
        "engine",
        "quarantine",
        "Staging",
        "Meter",
        "REGISTRY",
    ];
    for line in code_lines().filter(|line| !line.starts_with("use crate::engine::")) {
        for identifier in line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            assert!(
                !forbidden.contains(&identifier),
                "src/audit.rs names `{identifier}`: {line}"
            );
        }
    }
}
