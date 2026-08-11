//! G2-01 acceptance: what actually ships for the native protocol, and how tightly.
//!
//! > generated clients and schemas ship for the native protocol
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md`, G2
//!
//! # What this file is, and what it deliberately is not
//!
//! `notes/plan/notes/PHASE_A_EXIT_PACKAGE.md` re-ran the *delivering* suites from clean
//! state and read their results. This file does not do that: re-running
//! `idl_conformance.rs` and reading "ok" would be the delivering suite attesting to
//! itself. It re-derives the criterion's factual substrate by a different method —
//! **enumerate what ships, then measure what binds it** — and every number below is
//! computed from the tree at run time rather than quoted from that package.
//!
//! Three questions, in the order the criterion asks them:
//!
//! 1. **What ships?** [`INVENTORY`] names every client artifact and every schema
//!    artifact of the native protocol, each with how it was produced.
//!    [`the_inventory_method_reports_an_artifact_that_is_not_there`] is that method's own
//!    negative control.
//! 2. **Is any of it generated?** [`no_generator_exists_anywhere_in_the_workspace`]
//!    settles the mechanical half: no build script, no build dependency, no procedural
//!    macro crate, and no tool that reads the IDL and writes a file.
//! 3. **What binds the rest to the IDL, and where does that binding stop?** The
//!    `blindness` module derives, from the checkers' own sources and from the shipped
//!    spec types, five limits on what the two checkers can report — four that neither
//!    can see at all, and one where the redundancy of "two independent checkers" does
//!    not reach. `notes/plan/tools/g2_01_conformance_mutation_audit.py` is the empirical
//!    half: it applies each class as a real mutant on disk and records, per checker,
//!    whether it was caught, missed, or rejected by the compiler before any checker ran.
//!
//! # The reading this file does not make
//!
//! Whether a hand transcription held to the IDL by two independent checkers satisfies
//! "**generated** clients … ship" is a scope reading, and a reading is a decision. This
//! file supplies the facts on both sides and decides nothing: `Provenance::Generated`
//! exists in the vocabulary below and no artifact carries it, which is the fact, and
//! `every_operation_the_agent_grammar_names_is_declared` is the property a generator
//! would have given for free, held here by a check instead.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use continuumd::protocol::registry::OPERATIONS;

/// The repository root, reached from this crate's manifest.
const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// The normative wire authority, relative to [`REPO`].
const IDL: &str = "notes/plan/schemas/continuumd-native-protocol.idl";

/// The directory the schema documents ship from, relative to [`REPO`].
const SCHEMA_DIR: &str = "notes/plan/schemas";

/// Read a repository-relative file.
fn read(relative: &str) -> String {
    let path = Path::new(REPO).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{relative}: {error}"))
}

/// Whether a repository-relative path exists.
fn present(relative: &str) -> bool {
    Path::new(REPO).join(relative).exists()
}

/// Every file under a repository-relative directory, in a deterministic order.
fn walk(relative: &str) -> Vec<PathBuf> {
    let root = Path::new(REPO).join(relative);
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
            .map(|entry| entry.expect("a readable directory entry").path())
            .collect();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                pending.push(entry);
            } else {
                found.push(entry);
            }
        }
    }
    found.sort();
    found
}

// =====================================================================================
// 1. The inventory
// =====================================================================================

/// How a shipped artifact came to exist.
///
/// The four members are the whole answer space for "generated clients … ship", and they
/// are kept distinct because the criterion's force depends on the distinction. A
/// generator makes drift *impossible*; a checked transcription makes drift *reported*; a
/// hand-written artifact bound only through the types it names makes drift reported only
/// where a type is crossed; an unbound artifact makes drift silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provenance {
    /// The normative source itself — nothing produces it.
    Normative,
    /// Emitted from the IDL by the named generator.
    ///
    /// Never constructed today. It is declared so that the inventory's answer is a
    /// *value* rather than an absence of vocabulary: see
    /// [`no_generator_exists_anywhere_in_the_workspace`].
    #[expect(
        dead_code,
        reason = "the vocabulary is the finding; no artifact carries it"
    )]
    Generated(&'static str),
    /// Written by hand from the IDL and held to it by the named checkers.
    TranscribedByHand(&'static str),
    /// Written by hand, bound to the IDL only through the transcribed types it names.
    WrittenByHand,
}

/// One shipped artifact of the native protocol.
#[derive(Debug, Clone, Copy)]
struct Artifact {
    /// Repository-relative path.
    path: &'static str,
    /// What the artifact is on the protocol surface.
    role: &'static str,
    /// How it was produced.
    provenance: Provenance,
}

/// The name the two conformance checkers are cited by throughout this file.
const HELD_BY: &str =
    "crates/continuumd/tests/idl_conformance.rs + crates/continuumd/tests/registry_agreement.rs";

/// Every client artifact and every machine-contract artifact of the native protocol.
///
/// Schema *documents* are enumerated from the directory rather than listed here — there
/// are twenty and the directory is the authority — so this table carries the protocol's
/// code artifacts plus the two schema-side documents that are not `*.schema.json`.
const INVENTORY: &[Artifact] = &[
    // --- the authority ---------------------------------------------------------------
    Artifact {
        path: IDL,
        role: "the normative wire contract: 75 operations, 47 structs, 34 enums, 44 rules",
        provenance: Provenance::Normative,
    },
    Artifact {
        path: "notes/plan/schemas/README.md",
        role: "normative schema identity and versioning convention (document/class/instance)",
        provenance: Provenance::Normative,
    },
    // --- the transcription every client shares ---------------------------------------
    Artifact {
        path: "crates/continuumd/src/protocol/registry.rs",
        role: "the operation table: authority, annotations, bodies, errors, verdict, events",
        provenance: Provenance::TranscribedByHand(HELD_BY),
    },
    Artifact {
        path: "crates/continuumd/src/protocol/spec.rs",
        role: "the spec vocabulary and the `protocol_struct!` macro that emits type + FieldSpec",
        provenance: Provenance::TranscribedByHand(HELD_BY),
    },
    Artifact {
        path: "crates/continuumd/src/protocol/scalar.rs",
        role: "the IDL's scalars, handles and pattern-constrained aliases",
        provenance: Provenance::TranscribedByHand(HELD_BY),
    },
    Artifact {
        path: "crates/continuumd/src/protocol/vocabulary.rs",
        role: "the IDL's enums, member for member, with their wire spellings",
        provenance: Provenance::TranscribedByHand(HELD_BY),
    },
    Artifact {
        path: "crates/continuumd/src/protocol/envelope.rs",
        role: "RequestEnvelope, ResultEnvelope, Error, NextOperation, Budget, OutputPolicy",
        provenance: Provenance::TranscribedByHand(HELD_BY),
    },
    Artifact {
        path: "crates/continuumd/src/protocol/handshake.rs",
        role: "ClientHello, ServerWelcome, ServerReject and the version window",
        provenance: Provenance::TranscribedByHand(HELD_BY),
    },
    Artifact {
        path: "crates/continuumd/src/protocol/shared.rs",
        role: "the named structs several operations share",
        provenance: Provenance::TranscribedByHand(HELD_BY),
    },
    Artifact {
        path: "crates/continuumd/src/protocol/operations",
        role: "the per-namespace request and response bodies of all 75 operations",
        provenance: Provenance::TranscribedByHand(HELD_BY),
    },
    // --- the wire machinery every client shares --------------------------------------
    Artifact {
        path: "crates/continuumd/src/codec",
        role: "canonical JSON and canonical CBOR, both encodings the IDL declares",
        provenance: Provenance::WrittenByHand,
    },
    Artifact {
        path: "crates/continuumd/src/transport",
        role: "frame encode/decode and the in-process server; no socket ships",
        provenance: Provenance::WrittenByHand,
    },
    Artifact {
        path: "crates/continuumd/src/daemon/family.rs",
        role: "the closed `Arguments`/`Payload` families a client constructs calls from",
        provenance: Provenance::WrittenByHand,
    },
    // --- the agent client ------------------------------------------------------------
    Artifact {
        path: "crates/continuum-mcp/src/client.rs",
        role: "AgentClient: named methods over 9 operations, plus the generic `invoke`",
        provenance: Provenance::WrittenByHand,
    },
    Artifact {
        path: "crates/continuum-mcp/src/register.rs",
        role: "the machine-readable action grammar: 8 rows of operation + precondition",
        provenance: Provenance::WrittenByHand,
    },
    Artifact {
        path: "crates/continuum-mcp/src/answer.rs",
        role: "the client's typed answer, refusal and byte ledger",
        provenance: Provenance::WrittenByHand,
    },
    Artifact {
        path: "crates/continuum-mcp/src/link.rs",
        role: "the client's transport seam and its in-process link",
        provenance: Provenance::WrittenByHand,
    },
    // --- the CLI client --------------------------------------------------------------
    Artifact {
        path: "crates/continuum-cli/src/wire.rs",
        role: "the CLI's connection: every command funnels through it",
        provenance: Provenance::WrittenByHand,
    },
];

/// The inventory's own method: which entries name a path that is not there.
///
/// Separated from the assertion so that
/// [`the_inventory_method_reports_an_artifact_that_is_not_there`] can run it over a
/// deliberately broken inventory. A method with no negative control reports "nothing
/// missing" identically whether nothing is missing or nothing was looked at.
fn absent(entries: &[Artifact]) -> Vec<&'static str> {
    entries
        .iter()
        .filter(|entry| !present(entry.path))
        .map(|entry| entry.path)
        .collect()
}

/// Every schema document that ships, by file name.
fn schema_documents() -> BTreeSet<String> {
    walk(SCHEMA_DIR)
        .into_iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_owned();
            name.ends_with(".schema.json").then_some(name)
        })
        .collect()
}

#[test]
fn every_inventoried_artifact_is_present() {
    assert!(
        absent(INVENTORY).is_empty(),
        "the inventory names artifacts that do not ship: {:?}",
        absent(INVENTORY)
    );
    // The schema half, enumerated from the directory rather than from this file.
    let documents = schema_documents();
    assert_eq!(documents.len(), 20, "schema documents under {SCHEMA_DIR}");
    // Every document has a committed example instance; `validate_dossier.py` validates
    // each pair. The pairing is what makes a schema a *tested* contract rather than a
    // published intention, so its absence would be a half-shipped artifact.
    let examples: BTreeSet<String> = walk("notes/plan/schemas/examples")
        .into_iter()
        .filter_map(|path| Some(path.file_name()?.to_str()?.to_owned()))
        .collect();
    assert_eq!(examples.len(), 20, "example instances");
}

#[test]
fn the_inventory_method_reports_an_artifact_that_is_not_there() {
    // The negative control for section 1. One entry's path is perturbed; the method must
    // report that entry and only that entry.
    let mut broken: Vec<Artifact> = INVENTORY.to_vec();
    let victim = broken
        .iter_mut()
        .find(|entry| entry.path == "crates/continuum-mcp/src/client.rs")
        .expect("the agent client is inventoried");
    victim.path = "crates/continuum-mcp/src/client-that-does-not-ship.rs";

    assert_eq!(
        absent(&broken),
        vec!["crates/continuum-mcp/src/client-that-does-not-ship.rs"],
        "the inventory method must report an artifact that is not there"
    );
    assert!(
        absent(INVENTORY).is_empty(),
        "and must report nothing when everything is"
    );
}

#[test]
fn the_inventory_records_a_provenance_for_every_artifact_and_generates_none() {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in INVENTORY {
        let key = match entry.provenance {
            Provenance::Normative => "normative",
            Provenance::Generated(_) => "generated",
            Provenance::TranscribedByHand(_) => "transcribed-by-hand",
            Provenance::WrittenByHand => "written-by-hand",
        };
        *counts.entry(key).or_default() += 1;
        assert!(!entry.role.is_empty(), "{} states its role", entry.path);
    }
    assert_eq!(counts.get("generated"), None, "nothing here is generated");
    assert_eq!(counts["normative"], 2);
    assert_eq!(counts["transcribed-by-hand"], 8);
    assert_eq!(counts["written-by-hand"], 8);
    assert_eq!(INVENTORY.len(), 18);
}

// =====================================================================================
// 2. The generator half of the criterion
// =====================================================================================

#[test]
fn no_generator_exists_anywhere_in_the_workspace() {
    // Four mechanical ways a Rust workspace generates code, and the tree has none of
    // them. This is the factual core of the open reading: "generated" has no referent
    // here, so the criterion is either satisfied by the transcription or not satisfied.
    let mut build_scripts = Vec::new();
    let mut build_dependencies = Vec::new();
    let mut proc_macros = Vec::new();
    for path in walk("crates") {
        let relative = path
            .strip_prefix(Path::new(REPO))
            .expect("under the repository")
            .to_string_lossy()
            .into_owned();
        if relative.contains("/target/") {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) == Some("build.rs") {
            build_scripts.push(relative.clone());
        }
        if path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml") {
            let manifest = std::fs::read_to_string(&path).expect("a readable manifest");
            if manifest.contains("[build-dependencies]") {
                build_dependencies.push(relative.clone());
            }
            if manifest.contains("proc-macro") {
                proc_macros.push(relative);
            }
        }
    }
    assert!(build_scripts.is_empty(), "build scripts: {build_scripts:?}");
    assert!(
        build_dependencies.is_empty(),
        "build dependencies: {build_dependencies:?}"
    );
    assert!(proc_macros.is_empty(), "proc-macro crates: {proc_macros:?}");

    // The fourth way is a standalone tool. Every file outside `crates/` that names the
    // IDL by path is listed here, and each is a *reader*: none writes a file.
    let mut readers = Vec::new();
    for directory in ["tools", "notes/plan/tools"] {
        for path in walk(directory) {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.ends_with(".py") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("a readable tool");
            if source.contains("continuumd-native-protocol.idl") {
                let writes = source.contains(".write_text(")
                    || source.contains(".write_bytes(")
                    || source.contains("\"w\"")
                    || source.contains("'w'");
                readers.push((name.to_owned(), writes));
            }
        }
    }
    readers.sort();
    let names: Vec<&str> = readers.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        ["g2_01_conformance_mutation_audit.py", "validate_dossier.py"],
        "the tools that read the IDL"
    );
    // This file's own mutation harness writes — it restores the tree it mutates — so it
    // is excluded by name rather than by pretending otherwise.
    for (name, writes) in &readers {
        if name == "g2_01_conformance_mutation_audit.py" {
            continue;
        }
        assert!(!writes, "{name} reads the IDL and writes files");
    }
}

// =====================================================================================
// 3. What binds the transcription, and where the binding stops
// =====================================================================================

/// The two conformance checkers, by repository-relative path.
const CHECKERS: [&str; 2] = [
    "crates/continuumd/tests/idl_conformance.rs",
    "crates/continuumd/tests/registry_agreement.rs",
];

#[test]
fn the_two_conformance_checkers_ship_and_read_the_sources_they_claim() {
    let conformance = read(CHECKERS[0]);
    let agreement = read(CHECKERS[1]);

    // Leg one reads the IDL and parses it with a parser of its own.
    assert!(conformance.contains("continuumd-native-protocol.idl"));
    assert!(
        conformance.contains("pub fn tokenize") && conformance.contains("pub fn parse"),
        "leg one carries its own parser rather than sharing the types' one"
    );
    // Leg two reads two RFCs the IDL does not own, and `continuum-value`.
    assert!(agreement.contains("0027-agent-tool-protocol.md"));
    assert!(agreement.contains("0026-continuumd-native-protocol.md"));
    assert!(agreement.contains("continuum_value::"));

    // Independence, as a property of the sources rather than of their prose. Each file
    // cites the other by name in its own header — that is the design being described,
    // not a shared implementation — so the checkable property is that they share no code
    // and no source document: leg two never reads the IDL, leg one never reads either
    // RFC, and neither includes the other's text.
    assert!(
        !agreement.contains("continuumd-native-protocol.idl"),
        "leg two must not read the IDL, or its agreement is with the same document"
    );
    assert!(!conformance.contains("0027-agent-tool-protocol.md"));
    assert!(!conformance.contains("0026-continuumd-native-protocol.md"));
    for checker in CHECKERS {
        let source = read(checker);
        assert!(
            !source.contains("include_str!"),
            "{checker} must not include another checker's text"
        );
    }
}

mod blindness {
    //! Four divergence classes neither conformance checker can report, each derived
    //! from the checkers' own text or from the shipped spec types rather than asserted.
    //!
    //! `notes/plan/tools/g2_01_conformance_mutation_audit.py` demonstrates the same four
    //! empirically, by putting each on disk and running both checkers against it. Two
    //! methods because a blindness argued from source can be wrong about what the code
    //! does, and a blindness argued from one mutant can be wrong about the class.

    use super::{CHECKERS, IDL, read};

    #[test]
    fn since_is_untranscribed_so_no_checker_can_compare_one() {
        // The IDL dates declarations with `@since("X.Y")`. Count the declaration sites:
        // the header's own explanatory line is a `//` comment and is not one.
        let idl = read(IDL);
        let sites = idl
            .lines()
            .filter(|line| line.contains("@since(\"") && !line.trim_start().starts_with("//"))
            .count();
        assert_eq!(sites, 14, "declaration sites carrying @since");

        // None of the seven spec types carries a field to compare them against. This is
        // structural, not a gap in a comparison: there is nothing on the shipped side to
        // compare, so no checker built on these types can ever report a stale @since.
        let spec = read("crates/continuumd/src/protocol/spec.rs");
        for name in [
            "OperationSpec",
            "FieldSpec",
            "StructSpec",
            "EnumSpec",
            "UnionSpec",
            "HandleSpec",
            "AliasSpec",
        ] {
            let start = spec
                .find(&format!("pub struct {name} {{"))
                .unwrap_or_else(|| panic!("spec.rs declares {name}"));
            let end = spec[start..].find("\n}\n").expect("the struct closes") + start;
            let body = &spec[start..end];
            assert!(
                !body.contains("since") && !body.contains("version"),
                "{name} carries a version field, so this blindness claim is stale"
            );
        }

        // And leg one drops the annotation explicitly, on the operation clause where it
        // would otherwise have survived the parse.
        let conformance = read(CHECKERS[0]);
        assert!(
            conformance.contains(r#".filter(|annotation| annotation != "since")"#),
            "leg one filters @since out of an operation's annotations"
        );
    }

    #[test]
    fn field_and_member_annotations_are_dropped_by_the_parser() {
        // `fn fields` and the enum-member loop both call `self.annotations()` and discard
        // the result, so `@pattern`, `@since`, `@redactable` and every other annotation
        // that sits on a *field* or an *enum member* is invisible to leg one. Only alias
        // patterns and operation-level annotations survive.
        let conformance = read(CHECKERS[0]);
        let fields = conformance
            .find("fn fields(&mut self) -> Vec<Field> {")
            .expect("leg one parses field lists");
        let body = &conformance[fields..fields + 700];
        assert!(
            body.contains("self.annotations();"),
            "the field parser calls annotations() and discards the value"
        );
        assert!(
            !body.contains("annotations: "),
            "and stores nothing from it on the Field"
        );
        // The IDL really does carry field-level annotations, so the hole has content:
        // three fields declare `@since`, and every pattern-constrained alias member of a
        // struct relies on the alias declaration for its constraint.
        let idl = read(IDL);
        let annotated_fields = idl
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//")
                    && trimmed.contains(": ")
                    && trimmed.contains('@')
                    && trimmed.ends_with(';')
            })
            .count();
        assert_eq!(annotated_fields, 3, "fields carrying an annotation");
    }

    #[test]
    fn rule_bodies_are_dropped_except_the_one_rule_leg_one_reads_by_hand() {
        // The parser skips `"""` blocks, so the 44 rules' normative bodies are compared
        // against nothing. Leg one reaches back into the raw text for exactly one of
        // them — `errors.common` — which is the measure of the hole: 43 rule bodies,
        // including `encoding.opaque_payloads` and `conformance.registry_agreement`
        // themselves, are unchecked prose as far as both checkers are concerned.
        let idl = read(IDL);
        let rules = idl.lines().filter(|line| line.starts_with("rule ")).count();
        assert_eq!(rules, 44, "rules the IDL declares");

        let conformance = read(CHECKERS[0]);
        assert!(conformance.contains("dropping comments and `\"\"\"` blocks"));
        let read_by_hand: Vec<&str> = conformance
            .match_indices("rule_body(\"")
            .map(|(at, _)| {
                let rest = &conformance[at + "rule_body(\"".len()..];
                &rest[..rest.find('"').expect("a quoted rule name")]
            })
            .collect();
        assert_eq!(read_by_hand, ["errors.common"], "rule bodies read at all");
    }

    #[test]
    fn field_level_conformance_rests_on_one_checker_not_two() {
        // "Two independent conformance checkers" is true of the *registry row* and not of
        // the field. RFC 0027's table has an operation column, an authority column and an
        // annotations column, and no field column, so leg two cannot have an opinion
        // about a renamed field, a changed type, a changed presence marker or a dropped
        // error code — and the mutation audit measures exactly that: those four mutants
        // are `caught / blind`. That is by construction, not a defect, but it means the
        // redundancy the phrase "two independent checkers" suggests does not extend to
        // the largest part of the wire contract.
        let agreement = read(CHECKERS[1]);
        for symbol in [
            "FieldSpec",
            "NAMED_STRUCTS",
            "spec.request",
            "spec.response",
            "ENUMS",
            "UNIONS",
            "HANDLES",
            "ALIASES",
        ] {
            assert!(
                !agreement.contains(symbol),
                "leg two reads {symbol}; this blindness claim is stale"
            );
        }
        // What it does read: the row, and the vocabularies RFC 0026 binds to
        // `continuum-value`.
        for symbol in [
            "spec.authority",
            "spec.annotations",
            "spec.verdict",
            "spec.errors",
        ] {
            assert!(agreement.contains(symbol), "leg two reads {symbol}");
        }
    }

    #[test]
    fn neither_checker_reads_the_agent_client_or_the_cli() {
        // The strongest of the four. Both checkers compare the *transcription* to the
        // IDL. Neither compiles, includes, or names `continuum-mcp` or `continuum-cli`,
        // so no drift in a shipped client — a grammar row naming an operation that does
        // not exist, a named method putting a different operation on the wire — is
        // reachable by either. `crates/continuumd/tests/gate_g2_01_acceptance.rs`'s
        // section 4 is where that binding is made, and it is made here for the first
        // time.
        for checker in CHECKERS {
            let source = read(checker);
            for name in [
                "continuum-mcp",
                "continuum_mcp",
                "AgentClient",
                "continuum-cli",
                "continuum_cli",
            ] {
                assert!(
                    !source.contains(name),
                    "{checker} names {name}; this blindness claim is stale"
                );
            }
        }
    }
}

// =====================================================================================
// 4. The client half: the binding neither checker performs
// =====================================================================================

/// The `Arguments` variant to wire-name map, parsed from the daemon's own family.
///
/// Read from the source rather than from the type, because the point is to check what
/// the *clients* spell and a client spells a variant, not a name.
fn argument_variants() -> BTreeMap<String, String> {
    let family = read("crates/continuumd/src/daemon/family.rs");
    let start = family
        .find("pub const fn operation(&self) -> &'static str {")
        .expect("the Arguments family maps variants to wire names");
    let end = family[start..].find("\n    }\n").expect("the match closes") + start;
    let mut mapping = BTreeMap::new();
    for line in family[start..end].lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("Self::") else {
            continue;
        };
        let Some((variant, tail)) = rest.split_once("(_) => \"") else {
            continue;
        };
        let Some((name, _)) = tail.split_once('"') else {
            continue;
        };
        mapping.insert(variant.to_owned(), name.to_owned());
    }
    mapping
}

/// Every `Arguments::` variant a crate's sources construct.
fn variants_used_by(crate_source_dir: &str) -> BTreeSet<String> {
    let mut used = BTreeSet::new();
    for path in walk(crate_source_dir) {
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("a readable source file");
        for (at, _) in source.match_indices("Arguments::") {
            let rest = &source[at + "Arguments::".len()..];
            let variant: String = rest
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if !variant.is_empty() {
                used.insert(variant);
            }
        }
    }
    used
}

#[test]
fn every_operation_the_agent_grammar_names_is_declared() {
    // research/25's action grammar is the artifact an orchestrator plans against, and it
    // names operations as string literals. Neither conformance checker reads this file,
    // so until now nothing held those eight strings to the registry: a misspelling would
    // make `admits` fall through to "admitted" — the register admits what it does not
    // know — and the grammar would silently stop constraining the row it named.
    let register = read("crates/continuum-mcp/src/register.rs");
    let declared: BTreeSet<&str> = OPERATIONS.iter().map(|spec| spec.name).collect();

    let mut named = Vec::new();
    for (at, _) in register.match_indices("operation: \"") {
        let rest = &register[at + "operation: \"".len()..];
        named.push(&rest[..rest.find('"').expect("a quoted operation name")]);
    }
    assert_eq!(named.len(), 8, "the grammar's rows");
    for operation in &named {
        assert!(
            declared.contains(operation),
            "the agent grammar names {operation:?}, which the protocol does not declare"
        );
    }
    // The grammar's rows are a subset of the client's named surface, never a superset:
    // "a grammar naming an operation the client cannot call would be a promise about a
    // surface that is not there" (its own module documentation).
    let client_operations = client_operations();
    for operation in &named {
        assert!(
            client_operations.contains(*operation),
            "the grammar names {operation:?}, which AgentClient cannot call"
        );
    }
}

/// The wire operations `AgentClient`'s named methods construct.
fn client_operations() -> BTreeSet<String> {
    let mapping = argument_variants();
    variants_used_by("crates/continuum-mcp/src")
        .into_iter()
        .filter_map(|variant| mapping.get(&variant).cloned())
        .collect()
}

#[test]
fn every_operation_a_shipped_client_spells_is_declared() {
    let mapping = argument_variants();
    let declared: BTreeSet<&str> = OPERATIONS.iter().map(|spec| spec.name).collect();

    // The typed argument family itself: 30 of the registry's 75 operations have a
    // constructible request. The other 45 are reachable only as a name, which is the
    // accounting `rule errors.unsupported_surface` describes and not a client defect —
    // but it is the ceiling on what any client can spell today, so it is measured here.
    assert_eq!(mapping.len(), 30, "constructible request families");
    for name in mapping.values() {
        assert!(
            declared.contains(name.as_str()),
            "the Arguments family spells {name:?}, which the protocol does not declare"
        );
    }
    assert_eq!(OPERATIONS.len(), 75);

    for (crate_name, source_dir, expected) in [
        ("continuum-mcp", "crates/continuum-mcp/src", 9usize),
        ("continuum-cli", "crates/continuum-cli/src", 11usize),
    ] {
        let used = variants_used_by(source_dir);
        let mut operations = BTreeSet::new();
        for variant in &used {
            let name = mapping.get(variant).unwrap_or_else(|| {
                panic!(
                    "{crate_name} constructs Arguments::{variant}, which the family has no name for"
                )
            });
            assert!(
                declared.contains(name.as_str()),
                "{crate_name} calls {name:?}, which the protocol does not declare"
            );
            operations.insert(name.clone());
        }
        assert_eq!(operations.len(), expected, "{crate_name} named surface");
    }
}

#[test]
fn the_named_client_surfaces_are_a_small_and_stated_slice_of_the_registry() {
    // The numbers, pinned so that a client that grows is visible here. `AgentClient`'s
    // own module documentation still says "eight typed operations"; the ninth is
    // `workspace.create_by_reference`, landed at protocol 3.6. `invoke` is public and
    // takes the closed `Arguments` enum, so neither client is *limited* to its named
    // methods — the ceiling is the family's 30, not the 9 or the 11.
    let mapping = argument_variants();
    let mcp: BTreeSet<&String> = variants_used_by("crates/continuum-mcp/src")
        .iter()
        .filter_map(|variant| mapping.get(variant))
        .collect();
    let expected: BTreeSet<String> = [
        "task.cancel",
        "task.resume",
        "task.status",
        "verification.result",
        "verification.start",
        "workspace.create",
        "workspace.create_by_reference",
        "workspace.fork",
        "workspace.seal",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(mcp.into_iter().cloned().collect::<BTreeSet<_>>(), expected);
}

// =====================================================================================
// 5. The schema half: "and schemas ship"
// =====================================================================================

/// Every `schemas/<name>.schema.json` citation in the IDL, in first-appearance order.
fn schemas_cited_by_the_idl() -> BTreeSet<String> {
    let idl = read(IDL);
    let mut cited = BTreeSet::new();
    for (at, _) in idl.match_indices("schemas/") {
        let rest = &idl[at + "schemas/".len()..];
        let name: String = rest
            .chars()
            .take_while(|character| {
                character.is_ascii_lowercase() || *character == '-' || *character == '.'
            })
            .collect();
        if name.ends_with(".schema.json") {
            cited.insert(name);
        }
    }
    cited
}

/// Which of a set of cited schema names have no document under `notes/plan/schemas/`.
fn unresolved(cited: &BTreeSet<String>) -> Vec<String> {
    let documents = schema_documents();
    cited
        .iter()
        .filter(|name| !documents.contains(*name))
        .cloned()
        .collect()
}

#[test]
fn every_schema_the_idl_cites_resolves_to_a_document_that_ships() {
    let cited = schemas_cited_by_the_idl();
    assert_eq!(cited.len(), 17, "schema documents the IDL cites");
    assert!(
        unresolved(&cited).is_empty(),
        "the IDL cites schemas that do not ship: {:?}",
        unresolved(&cited)
    );

    // Negative control for this method: a citation with no document must be reported.
    let mut fabricated = cited.clone();
    fabricated.insert("no-such-artifact.schema.json".to_owned());
    assert_eq!(
        unresolved(&fabricated),
        vec!["no-such-artifact.schema.json".to_owned()],
        "the resolver must report a citation that does not resolve"
    );
}

#[test]
fn three_shipped_schema_documents_are_cited_nowhere_in_the_idl() {
    // The reverse direction. Twenty documents ship; the IDL reaches seventeen. The three
    // it never names are not defects of the schema directory — `corpus-port` governs a
    // dossier corpus port and `domain-pack` an RFC 0002 manifest, neither of which is a
    // wire payload — but `synthesis-candidate` is: `forge.create`'s `sketch: Opaque` is
    // the payload it governs, and the IDL does not say so.
    let cited = schemas_cited_by_the_idl();
    let uncited: Vec<String> = schema_documents()
        .into_iter()
        .filter(|name| !cited.contains(name))
        .collect();
    assert_eq!(
        uncited,
        [
            "corpus-port.schema.json",
            "domain-pack.schema.json",
            "synthesis-candidate.schema.json",
        ]
    );
}

#[test]
fn every_shipped_schema_document_declares_the_readme_identity_triple() {
    // `notes/plan/schemas/README.md` is normative for schema identity: a document
    // declares `$id = https://continuum.dev/schema/v<epoch>/<name>.json`, a matching
    // `schema_epoch`, and a `schema_kind`. Checked here without a JSON parser — the
    // three declarations are exact strings and the `$id` is a function of the file name.
    for name in schema_documents() {
        let source = read(&format!("{SCHEMA_DIR}/{name}"));
        let stem = name
            .strip_suffix(".schema.json")
            .expect("a schema document file name");
        // The document-level declaration, not the `properties` entry that describes the
        // keyword: the first occurrence whose value is a number rather than a subschema.
        let epoch: String = source
            .match_indices("\"schema_epoch\":")
            .find_map(|(at, marker)| {
                let digits: String = source[at + marker.len()..]
                    .trim_start()
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                (!digits.is_empty()).then_some(digits)
            })
            .unwrap_or_else(|| panic!("{name} declares a numeric schema_epoch"));
        let expected = format!("\"https://continuum.dev/schema/v{epoch}/{stem}.json\"");
        assert!(
            source.contains(&expected),
            "{name} must declare $id {expected}"
        );
        assert!(
            source.contains("\"schema_kind\": \"artifact\"")
                || source.contains("\"schema_kind\": \"value\""),
            "{name} declares a schema_kind"
        );
        // The retired identity forms the README names are not accepted.
        assert!(
            !source.contains("continuum.dev/schemas/"),
            "{name}: pre-R3 $id"
        );
    }
}

/// One `Opaque`-typed field in the IDL and the schema its documentation names.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OpaqueField {
    /// The declaration that encloses it, as the IDL writes it.
    declaration: String,
    /// The field name.
    field: String,
    /// The schema its own doc comment names, if any.
    own_doc: Option<String>,
    /// The schema the enclosing declaration's doc comment names, if any.
    enclosing_doc: Option<String>,
}

/// Every `Opaque` field the IDL declares, with the schema citations around it.
fn opaque_fields() -> Vec<OpaqueField> {
    let idl = read(IDL);
    let lines: Vec<&str> = idl.lines().collect();
    let schema_in = |text: &str| -> Option<String> {
        let at = text.find("schemas/")?;
        let rest = &text[at + "schemas/".len()..];
        let name: String = rest
            .chars()
            .take_while(|character| {
                character.is_ascii_lowercase() || *character == '-' || *character == '.'
            })
            .collect();
        name.ends_with(".schema.json").then_some(name)
    };
    let docs_above = |mut index: usize| -> String {
        let mut collected = Vec::new();
        while index > 0 && lines[index - 1].trim_start().starts_with("///") {
            index -= 1;
            collected.push(lines[index].trim_start().trim_start_matches("///").trim());
        }
        collected.join(" ")
    };

    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        let Some((field, tail)) = trimmed.split_once(": ") else {
            continue;
        };
        if !tail.starts_with("Opaque ") || !trimmed.ends_with(';') {
            continue;
        }
        if !field
            .chars()
            .all(|character| character.is_ascii_lowercase() || character == '_')
        {
            continue;
        }
        // The enclosing declaration, and the doc comment above it.
        let mut at = index;
        while at > 0 {
            let candidate = lines[at];
            if candidate.starts_with("operation ")
                || candidate.starts_with("struct ")
                || candidate.starts_with("union ")
            {
                break;
            }
            at -= 1;
        }
        let mut header = at;
        while header > 0 && lines[header - 1].trim_start().starts_with('@') {
            header -= 1;
        }
        found.push(OpaqueField {
            declaration: lines[at].trim_end_matches(" {").to_owned(),
            field: field.to_owned(),
            own_doc: schema_in(&docs_above(index)),
            enclosing_doc: schema_in(&docs_above(header)),
        });
    }
    found
}

#[test]
fn most_structured_payloads_on_the_wire_name_no_schema_at_all() {
    // `rule encoding.opaque_payloads` is the IDL's own statement of the schema half:
    //
    // > every other `Opaque` names a schema in `schemas/` in its doc comment and is
    // > carried verbatim as a canonical value of the negotiated encoding
    //
    // "every other" excludes the four envelope payloads, whose shape is resolved from
    // the message's own `operation`/`code` field. This measures the rest. Neither
    // conformance checker can: the parser drops doc comments and drops rule bodies, so
    // the sentence that states the requirement and the comments that would satisfy it
    // are both outside everything they compare.
    let fields = opaque_fields();
    assert_eq!(fields.len(), 30, "Opaque fields the IDL declares");

    let envelope: Vec<&OpaqueField> = fields
        .iter()
        .filter(|item| {
            matches!(
                item.declaration.as_str(),
                "struct RequestEnvelope"
                    | "struct ResultEnvelope"
                    | "struct NextOperation"
                    | "struct Error"
            )
        })
        .collect();
    assert_eq!(envelope.len(), 4, "the envelope-resolved payloads");

    let governed: Vec<&OpaqueField> = fields
        .iter()
        .filter(|item| !envelope.contains(item))
        .collect();
    assert_eq!(governed.len(), 26, "payloads the rule governs");

    let strict = governed
        .iter()
        .filter(|item| item.own_doc.is_some())
        .count();
    let lenient = governed
        .iter()
        .filter(|item| item.own_doc.is_none() && item.enclosing_doc.is_some())
        .count();
    let unnamed: Vec<String> = governed
        .iter()
        .filter(|item| item.own_doc.is_none() && item.enclosing_doc.is_none())
        .map(|item| format!("{}.{}", item.declaration, item.field))
        .collect();

    assert_eq!(strict, 7, "fields whose own doc comment names a schema");
    assert_eq!(
        lenient, 1,
        "fields reached only through the enclosing declaration's doc comment"
    );
    assert_eq!(
        unnamed,
        [
            "operation intent.diff.summary",
            "operation intent.accept.acceptance",
            "operation intent.accept.record",
            "operation model.compare.summary",
            "operation program.run.configuration",
            "operation program.replay.divergence",
            "operation proof.goal.goal",
            "operation debug.open.frontier",
            "operation debug.state.state",
            "operation debug.step_event.delta",
            "operation debug.step_abstract.delta",
            "operation debug.reverse_causal.delta",
            "operation context.expand.pack",
            "operation repair.apply.changes",
            "operation observe.classify.classification",
            "operation forge.create.sketch",
            "operation evidence.get.node",
            "operation evidence.get.edge",
        ],
        "18 of the 26 structured payloads the rule governs name no schema anywhere in \
         their declaration; the machine contract for those bytes is not reachable from \
         the IDL, which is the measured shape of the criterion's second half"
    );
}
