//! Evidence for `continuum explain compile` (bn-3rqvm, PR-13 first command group) — the
//! Context Pack projection.
//!
//! # What this file is evidence for
//!
//! `explain compile` drives `context.compile`, which `continuumd::daemon::context::ContextFamily`
//! **registers, serves the namespace of, and refuses**: "no Context Pack compiler is served by
//! this daemon". So the obligation here is two-sided, and both sides are tested against the
//! real wire types:
//!
//! 1. **INV-008 at the interface** — the refusal is typed, machine-readable, names exactly
//!    which operation is unsupported, and is never a crash, an opaque failure, or a
//!    fabricated pack. Live, over a real frame, against a daemon that *does* serve the
//!    `context` namespace, so the answer is the family's own statement about a compiler
//!    rather than a dispatcher's statement about a missing namespace.
//! 2. **the projection, proven before the compiler lands** — the success renderer against a
//!    real `ContextCompileResponse` carrying a conforming
//!    `schemas/context-pack.schema.json` document. Same reasoning bn-3tz60 gave for
//!    `context.expand` and bn-1g7e4 for `debug`/`repair`.
//!
//! # Clause → test
//!
//! - **a Context Pack projection** → [`the_projection_reads_every_scalar_the_pack_declares`],
//!   [`the_projection_prints_both_manifests_and_conflates_neither`].
//! - **prose is a projection of the JSON, never a separate truth (INV-003, plan §3.1)** →
//!   [`every_value_the_text_projection_prints_is_in_the_json_document_it_projects`].
//! - **a document the daemon did not send is not invented** →
//!   [`an_unparseable_pack_reads_none_beside_a_byte_count_that_is_not_none`] and
//!   [`a_pack_missing_a_field_reads_none_rather_than_a_guess`].
//! - **typed depth, live** →
//!   [`the_unsupported_compiler_is_the_familys_own_typed_refusal`] and the two golden pins.
//! - **INV-002 / usage** → [`explain_compile_requires_its_three_declarations`].
//! - **adversarial input, linear** → [`a_large_question_is_still_a_typed_answer`].

use continuum_cli::explain::{self, CompileArgs};
use continuum_cli::format::Format;
use continuum_cli::render::Depth;
use continuum_cli::wire::{Admitted, Connection, LinkError, LocalLink, Outcome, Transport};
use continuum_cli::{cli, explain as explain_module};
use continuum_value::epoch::ProtocolWindow;
use continuumd::daemon::Daemon;
use continuumd::daemon::context::ContextFamily;
use continuumd::daemon::family::Payload;
use continuumd::daemon::identity::Blake3Identity;
use continuumd::protocol::envelope::{ArtifactRef, Cost, EpochSet, NextOperation, Omission};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::ContextCompileResponse;
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, ArtifactHandle, CapabilityHandle, ContextHandle, EpochIdentity, Opaque,
    ProtocolVersion, RequestId, Timestamp,
};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    Audience, AuthorityLevel, Encoding, ErrorCode, OmissionReason, ResultStatus,
};
use continuumd::transport::{LocalPair, Server};

const NOW: &str = "2026-08-01T00:00:00.000Z";

/// A conforming Context Pack — the artifact `context.compile` answers with once a compiler
/// exists. Keys are in ascending code-point order because that is the canonical form the
/// protocol's own encoding defines, and `Json::parse` reads no other spelling.
const PACK: &str = r#"{"assurance":{"class":"bounded","envelope":{}},"content_budget":{"bytes":4096,"nodes":12},"content_hash":"blake3-256:packplaceholder","context_id":"ctx_explained00001","evidence":["ev_failure1","ev_replay1"],"expansions":[{"anchor":"node-42","estimated_items":2,"relation":"source_span"}],"guarantees":["ReplayPreserving"],"inconclusive_reason":"ResourceExhausted","intent":"in_ack_v1","omissions":[{"count":2,"expandable":true,"expansion":{"anchor":"node-42","relation":"source_span"},"kind":"source","reason":"budget"},{"count":1,"expandable":false,"kind":"model","reason":"redaction"}],"parent":null,"question":"why did AckImpliesDurable fail?","redactions":[{"commitment":"blake3-256:withheld1","original_class":"evidence","reason":"purged","redacted":true}],"replay":"crash_demo1","schema_epoch":1,"schema_id":"https://continuum.dev/schema/context-pack.json","selected":[{"artifact":"ev_ack1","id":"node-42","kind":"event","summary":"reply published before stable write"}],"semantic_epoch":"sem3-r3-demo","snapshot":"ws_demo1","verdict":"inconclusive"}"#;

// --- fixture, duplicated locally --------------------------------------------------------

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 2)
}

fn actor(name: &str) -> ActorId {
    ActorId::new(name).expect("a well-formed actor identity")
}

fn capability(name: &str) -> CapabilityHandle {
    CapabilityHandle::new(name).expect("a well-formed capability handle")
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Null,
        proof: Nullable::Value(epoch("proof-1")),
        corpus: Nullable::Null,
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
}

fn hello() -> ClientHello {
    ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuum-cli-evidence".to_owned(),
        actor: actor("service:continuumd"),
        capability: capability("cap_root"),
        features: Optional::Absent,
    }
}

fn root_grant() -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: capability("cap_root"),
        actor: actor("service:continuumd"),
        level: AuthorityLevel::Promote,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 4,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: Vec::new(),
            denied_operations: Vec::new(),
            data_grants: Vec::new(),
            cross_principal_sharing: true,
        }),
        instances: Optional::Absent,
    }
}

/// A deployment that serves the `context` namespace — so a `context.compile` refusal is the
/// family's own statement about a compiler, not a dispatcher's about a missing namespace.
struct Fixture {
    server: Server,
    pair: LocalPair,
}

impl Fixture {
    fn fresh() -> Self {
        let hello = hello();
        let negotiated = negotiate(
            &[
                ProtocolVersion::new(3, 0),
                ProtocolVersion::new(3, 1),
                version(),
            ],
            ProtocolWindow::new(3),
            ENCODINGS,
            &hello,
        )
        .expect("3.2 is served");
        let daemon = Daemon::builder(Blake3Identity, negotiated, capability("cap_root"))
            .epochs(epochs())
            .now(Timestamp::new(NOW).expect("a timestamp"))
            .capability(root_grant(), None)
            .family(ContextFamily)
            .build();
        Self {
            server: Server::new(daemon, negotiated),
            pair: LocalPair::new(),
        }
    }

    fn link(&mut self) -> LocalLink<'_> {
        LocalLink::new(&mut self.server, &mut self.pair)
    }
}

fn fresh_connection() -> Connection {
    Connection::new(
        version(),
        actor("service:continuumd"),
        capability("cap_root"),
    )
}

fn compile_args(question: &str) -> CompileArgs {
    CompileArgs {
        evidence_root: ArtifactHandle::new("ev_failure1").expect("an artifact handle"),
        question: question.to_owned(),
        audience: Optional::Present(Audience::Agent),
        guarantees: vec!["ReplayPreserving".to_owned()],
        bytes: 4096,
    }
}

/// One `key  value` line's value.
fn line(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{key}  ")))
        .unwrap_or_else(|| panic!("no {key:?} line in:\n{text}"))
        .to_owned()
}

/// An admitted `context.compile` answer carrying `pack`.
fn admitted(pack: &[u8], omissions: Vec<Omission>) -> Outcome<Payload> {
    Outcome::Admitted(Admitted {
        request_id: RequestId::new("req_cli000001").expect("a request id"),
        status: ResultStatus::Ok,
        payload: Payload::ContextCompile(ContextCompileResponse {
            context: ContextHandle::new("ctx_explained00001").expect("a context handle"),
            pack: Opaque::from_bytes(pack.to_vec()),
        }),
        verdict: None,
        assurance: None,
        task: None,
        continuation: None,
        omissions,
        artifacts: Vec::<ArtifactRef>::new(),
        cost: Cost {
            wall_ms: Optional::Absent,
            cpu_ms: Optional::Absent,
            memory_bytes: Optional::Absent,
            states: Optional::Absent,
            solver_ms: Optional::Absent,
            proof_ms: Optional::Absent,
            tokens: Optional::Absent,
            candidates: Optional::Absent,
            bytes: Optional::Absent,
            tokenizer_id: Optional::Absent,
        },
        next_operations: Vec::<NextOperation>::new(),
    })
}

// --- the live refusal ---------------------------------------------------------------------

#[test]
fn the_unsupported_compiler_is_the_familys_own_typed_refusal() {
    assert!(
        registry::operation(explain::COMPILE_OPERATION).is_some(),
        "context.compile is a registered operation, so an unsupported answer is about the \
         deployment and not about the name"
    );
    let mut fixture = Fixture::fresh();
    for format in [Format::Text, Format::Pretty, Format::Json] {
        let mut connection = fresh_connection();
        let mut link = fixture.link();
        let rendered = explain::compile(&mut connection, &mut link, &compile_args("why?"), format)
            .expect("an unserved compiler is a rendered answer, never a CLI error");
        assert_eq!(rendered.exit_code, 1, "{format:?}:\n{}", rendered.text);
        assert!(
            rendered.text.contains(Depth::Unsupported.token()),
            "{format:?} names the depth:\n{}",
            rendered.text
        );
        assert!(
            rendered.text.contains(explain::COMPILE_OPERATION),
            "{format:?} names exactly which operation is unsupported:\n{}",
            rendered.text
        );
        assert!(
            rendered
                .text
                .contains(ErrorCode::UnsupportedSemanticFeature.as_wire()),
            "{format:?} carries the protocol's own wire token:\n{}",
            rendered.text
        );
        // The family's own sentence, not the dispatcher's: this deployment *serves* the
        // `context` namespace and both of its operations, and refuses *this evidence root*
        // in particular, because no candidate order is registered for it (bn-1y4qc).
        assert!(
            rendered
                .text
                .contains("no candidate order for that evidence root"),
            "{format:?} carries the family's own detail:\n{}",
            rendered.text
        );
    }
}

/// The whole `explain compile --json` document on the arm this deployment reaches.
#[test]
fn the_unsupported_json_document_is_pinned_byte_for_byte() {
    let mut fixture = Fixture::fresh();
    let mut connection = fresh_connection();
    let mut link = fixture.link();
    let rendered = explain::compile(
        &mut connection,
        &mut link,
        &compile_args("why did AckImpliesDurable fail?"),
        Format::Json,
    )
    .expect("the call is answered");
    assert_eq!(
        rendered.text,
        concat!(
            r#"{"advice":[],"artifacts":[],"audience":"agent","budget_bytes":4096,"#,
            r#""context":null,"cost":null,"depth":"unsupported","#,
            // `data` joined the refusal at protocol 3.4 (RFC 0026 F19, bn-3jrtz):
            // null for every code that declares no `Error.data` shape.
            r#""error":{"code":"UnsupportedSemanticFeature","continuation":null,"data":null,"#,
            r#""detail":"this daemon holds no candidate order for that evidence root; "#,
            r#"stage 2's input is a CIR causal order and no subsystem here derives one "#,
            r#"from an evidence graph",""#,
            r#"non_resumable_reason":null,"recovery":[],"retryable":false},"#,
            r#""evidence_root":"ev_failure1","guarantees":["ReplayPreserving"],"#,
            r#""next_operations":[],"omissions":[],"#,
            r#""operation":"context.compile","pack":null,"pack_bytes":null,"#,
            r#""question":"why did AckImpliesDurable fail?","request_id":"req_cli000001"}"#,
            "\n"
        ),
        "the machine envelope is canonical JSON with a fixed key set"
    );
    assert_eq!(rendered.exit_code, 1);
}

// --- the projection, proven before the compiler lands -------------------------------------

#[test]
fn the_projection_reads_every_scalar_the_pack_declares() {
    let outcome = admitted(PACK.as_bytes(), Vec::new());
    let rendered = explain::render_compile(&compile_args("why?"), &outcome, Format::Text);
    assert_eq!(rendered.exit_code, 0, "{}", rendered.text);
    assert_eq!(line(&rendered.text, "depth"), "served");
    assert_eq!(line(&rendered.text, "context"), "ctx_explained00001");

    for (key, value) in [
        ("pack.context_id", "ctx_explained00001"),
        ("pack.snapshot", "ws_demo1"),
        ("pack.intent", "in_ack_v1"),
        ("pack.question", "why did AckImpliesDurable fail?"),
        ("pack.verdict", "inconclusive"),
        ("pack.inconclusive_reason", "ResourceExhausted"),
        ("pack.assurance.class", "bounded"),
        ("pack.content_budget.bytes", "4096"),
        ("pack.content_budget.nodes", "12"),
        ("pack.content_hash", "blake3-256:packplaceholder"),
        ("pack.semantic_epoch", "sem3-r3-demo"),
        ("pack.replay", "crash_demo1"),
        ("pack.schema_epoch", "1"),
        // Declared and null in the document: a root pack has no parent, and the projection
        // says so rather than dropping the line.
        ("pack.parent", "none"),
        // Not in this document at all: also `none`, and the JSON beside it is the authority
        // on which of the two it was.
        ("pack.debugger_branch", "none"),
        // A tokens count is absent, so no tokenizer identity is claimed for it.
        ("pack.content_budget.tokens", "none"),
        ("pack.content_budget.tokenizer_id", "none"),
    ] {
        assert_eq!(line(&rendered.text, key), value, "{key}");
    }

    // Lists, unabridged and by name.
    assert_eq!(line(&rendered.text, "pack.selected"), "1");
    assert_eq!(line(&rendered.text, "pack.selected[0].id"), "node-42");
    assert_eq!(line(&rendered.text, "pack.selected[0].kind"), "event");
    assert_eq!(line(&rendered.text, "pack.selected[0].artifact"), "ev_ack1");
    assert_eq!(line(&rendered.text, "pack.evidence"), "2");
    assert_eq!(line(&rendered.text, "pack.evidence[1]"), "ev_replay1");
    assert_eq!(
        line(&rendered.text, "pack.guarantees[0]"),
        "ReplayPreserving"
    );
    assert_eq!(line(&rendered.text, "pack.expansions"), "1");
    assert_eq!(
        line(&rendered.text, "pack.expansions[0].relation"),
        "source_span"
    );
    // "Withheld is not absent": the pack's redaction stubs are rendered field by field.
    assert_eq!(line(&rendered.text, "pack.redactions"), "1");
    assert_eq!(line(&rendered.text, "pack.redactions[0].reason"), "purged");
    assert_eq!(
        line(&rendered.text, "pack.redactions[0].original_class"),
        "evidence"
    );
}

#[test]
fn the_projection_prints_both_manifests_and_conflates_neither() {
    // The pack's own RFC 0028 manifest is a statement about the *selection*; the envelope's
    // INV-007 manifest is a statement about the *answer*. Two obligations, two key sets, and
    // neither substitutes for the other.
    let outcome = admitted(
        PACK.as_bytes(),
        vec![Omission {
            reason: OmissionReason::Budget,
            subject: "context.depth".to_owned(),
            recoverable_by: Optional::Absent,
        }],
    );
    let rendered = explain::render_compile(&compile_args("why?"), &outcome, Format::Text);

    // The envelope's manifest.
    assert_eq!(line(&rendered.text, "omissions"), "1");
    assert_eq!(
        line(&rendered.text, "omissions[0].subject"),
        "context.depth"
    );

    // The pack's own — a different count, and every entry by kind, reason and exact count.
    assert_eq!(line(&rendered.text, "pack.omissions"), "2");
    assert_eq!(line(&rendered.text, "pack.omissions[0].kind"), "source");
    assert_eq!(line(&rendered.text, "pack.omissions[0].reason"), "budget");
    assert_eq!(line(&rendered.text, "pack.omissions[0].count"), "2");
    assert_eq!(line(&rendered.text, "pack.omissions[0].expandable"), "true");
    assert_eq!(
        line(&rendered.text, "pack.omissions[0].expansion.anchor"),
        "node-42",
        "an expandable omission names the query that retrieves it (INV-007)"
    );
    // An irretrievable one has no expansion, and the absence is stated rather than implied.
    assert_eq!(
        line(&rendered.text, "pack.omissions[1].expandable"),
        "false"
    );
    assert_eq!(
        line(&rendered.text, "pack.omissions[1].expansion.relation"),
        "none"
    );
}

#[test]
fn every_value_the_text_projection_prints_is_in_the_json_document_it_projects() {
    // INV-003 / plan §3.1 as a test: the prose is a *projection* of the machine answer and
    // never a separate truth. Every `pack.*` scalar the text format prints is a substring of
    // the canonical JSON document the `--format json` answer embeds — so there is no line a
    // reader could act on that a parser could not.
    let outcome = admitted(PACK.as_bytes(), Vec::new());
    let text = explain::render_compile(&compile_args("why?"), &outcome, Format::Text);
    let json = explain::render_compile(&compile_args("why?"), &outcome, Format::Json);

    let mut checked = 0;
    for rendered in text.text.lines() {
        let Some((key, value)) = rendered.split_once("  ") else {
            continue;
        };
        if !key.starts_with("pack.") || value == "none" {
            continue;
        }
        assert!(
            json.text.contains(value),
            "{key} = {value:?} is prose the machine answer does not carry:\n{}",
            json.text
        );
        checked += 1;
    }
    assert!(
        checked >= 20,
        "the projection printed only {checked} pack values; the fixture declares more"
    );

    // …and the document itself is embedded verbatim rather than summarized, so a machine
    // reads the pack without a second call.
    assert!(
        json.text.contains("\"context_id\":\"ctx_explained00001\""),
        "{}",
        json.text
    );
    assert!(json.text.contains("\"pack_bytes\":"), "{}", json.text);
}

#[test]
fn an_unparseable_pack_reads_none_beside_a_byte_count_that_is_not_none() {
    // A present-but-unreadable document is visibly different from an absent one: `(none, N)`
    // versus `(none, none)`. A projection that printed the same thing for both would hide a
    // malformed artifact behind a missing one.
    let outcome = admitted(b"not a document", Vec::new());
    let text = explain::render_compile(&compile_args("why?"), &outcome, Format::Text);
    assert_eq!(line(&text.text, "pack_bytes"), "14");
    assert_eq!(line(&text.text, "pack.context_id"), "none");
    assert_eq!(line(&text.text, "pack.selected"), "none");

    let json = explain::render_compile(&compile_args("why?"), &outcome, Format::Json);
    assert!(json.text.contains("\"pack\":null"), "{}", json.text);
    assert!(json.text.contains("\"pack_bytes\":14"), "{}", json.text);
}

#[test]
fn a_pack_missing_a_field_reads_none_rather_than_a_guess() {
    let outcome = admitted(br#"{"context_id":"ctx_minimal00000001"}"#, Vec::new());
    let text = explain::render_compile(&compile_args("why?"), &outcome, Format::Text);
    assert_eq!(line(&text.text, "pack.context_id"), "ctx_minimal00000001");
    for absent in [
        "pack.verdict",
        "pack.assurance.class",
        "pack.content_budget.bytes",
        "pack.question",
    ] {
        assert_eq!(line(&text.text, absent), "none", "{absent}");
    }
    // An absent array is `none` and not `0`: an empty manifest is an *assertion* under
    // RFC 0028, and the projection does not make it on the pack's behalf.
    assert_eq!(line(&text.text, "pack.omissions"), "none");
    assert_eq!(line(&text.text, "pack.selected"), "none");

    let empty = admitted(br#"{"omissions":[],"selected":[]}"#, Vec::new());
    let text = explain::render_compile(&compile_args("why?"), &empty, Format::Text);
    assert_eq!(line(&text.text, "pack.omissions"), "0");
    assert_eq!(line(&text.text, "pack.selected"), "0");
}

#[test]
fn the_pack_projection_is_total_over_an_answer_that_carries_no_pack() {
    // The refusal arm renders the same key set reading `none`, so text output differs
    // between arms in its values and never in its shape.
    let lines = explain_module::pack_lines(None);
    assert!(
        lines.iter().all(|(_, value)| value == "none"),
        "an absent document reads `none` in every key"
    );
    assert!(
        lines.iter().any(|(key, _)| key == "pack.context_id"),
        "the key set is the same one a present document renders"
    );
}

// --- usage and adversarial input ----------------------------------------------------------

struct Deaf;

impl Transport for Deaf {
    fn exchange(&mut self, _frame: &[u8]) -> Result<Vec<u8>, LinkError> {
        Err(LinkError::NoAnswer)
    }
}

fn args(words: &[&str]) -> Vec<String> {
    let mut line: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
    line.extend(
        ["--actor", "service:continuumd", "--capability", "cap_root"]
            .iter()
            .map(|word| (*word).to_owned()),
    );
    line
}

#[test]
fn explain_compile_requires_its_three_declarations() {
    // The evidence root, the question, and the byte ceiling: each is `required` on the wire
    // or required by `@task_starting`, and none is invented here.
    let lines: [&[&str]; 4] = [
        &["explain", "compile"],
        &["explain", "compile", "--evidence-root", "ev_failure1"],
        &[
            "explain",
            "compile",
            "--evidence-root",
            "ev_failure1",
            "--question",
            "why?",
        ],
        &[
            "explain",
            "compile",
            "--evidence-root",
            "ev_failure1",
            "--question",
            "why?",
            "--bytes",
            "4096",
            "--audience",
            "not-an-audience",
        ],
    ];
    for line in lines {
        let mut transport = Deaf;
        let error = cli::run(&args(line), &mut transport, false)
            .expect_err("an incomplete command never reaches the wire");
        assert_eq!(
            error.exit_code(),
            1,
            "{line:?} is a usage error, not a wire failure"
        );
    }

    // …and the complete line does reach it: exit `2` off a deaf transport is what proves the
    // parse succeeded.
    let mut transport = Deaf;
    let error = cli::run(
        &args(&[
            "explain",
            "compile",
            "--evidence-root",
            "ev_failure1",
            "--question",
            "why?",
            "--bytes",
            "4096",
        ]),
        &mut transport,
        false,
    )
    .expect_err("the deaf transport answers nothing");
    assert_eq!(error.exit_code(), 2);
}

/// A 64 KiB question — one `repeat`, so the fixture is linear in the size it names.
#[test]
fn a_large_question_is_still_a_typed_answer() {
    const QUESTION_BYTES: usize = 64 * 1024;
    let question = "q".repeat(QUESTION_BYTES);
    assert_eq!(question.len(), QUESTION_BYTES);

    let mut fixture = Fixture::fresh();
    let mut connection = fresh_connection();
    let mut link = fixture.link();
    let rendered = explain::compile(
        &mut connection,
        &mut link,
        &compile_args(&question),
        Format::Json,
    )
    .expect("a large argument is encoded, sent, and answered like any other");
    assert_eq!(rendered.exit_code, 1);
    assert!(rendered.text.contains("\"depth\":\"unsupported\""));
    // The question travelled verbatim and came back in the echoed request, untruncated: a
    // projection that silently shortened it would be inventing content.
    assert!(rendered.text.contains(&question));
}
