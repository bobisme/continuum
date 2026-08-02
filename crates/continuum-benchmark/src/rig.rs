//! The rig: one provisioned daemon, one connected pair, and nothing ambient.
//!
//! # What a rig is
//!
//! A [`Rig`] is a deployment: a [`Daemon`] with the four landed operation families, four
//! capabilities at four authority levels, both corpus ports staged and registered, the
//! governing intent accepted, and a completed handshake. Both arms of the G0-DX-10
//! comparison drive **one** rig shape, built by one function, so a difference in the
//! measurements is a difference in the interface and not in the daemon.
//!
//! # What does not cross a frame, and why
//!
//! Content staging, model registration and capability minting are out-of-band
//! administration, not operations (IDL §7, `Server::daemon_mut`'s own documentation), so
//! they happen before any connection exists. `intent.accept` is a genuine wire operation and
//! is still called directly through [`Daemon::dispatch`] here, for the same reason
//! `continuumd`'s own transport tests do: accepting the governing intent is a *precondition*
//! of `workspace.create`, not a step either arm is being measured on. Charging one arm for
//! it and not the other would be an accounting error; charging both would be measuring the
//! deployment.
//!
//! # Determinism (INV-005)
//!
//! Nothing here reads a clock, an environment variable, a random number, or a file at run
//! time. The corpus bytes are `include_str!`'d, the timestamp is a constant, the capability
//! table is a literal, and the epochs are a literal. [`Rig::fresh`] called twice produces two
//! daemons that answer the same script with the same bytes — which is the device
//! `crate::run`'s reproduction check depends on, and is asserted rather than assumed.
//!
//! [`Daemon`]: continuumd::daemon::Daemon
//! [`Daemon::dispatch`]: continuumd::daemon::Daemon::dispatch

use std::collections::BTreeMap;

use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::ContentIdentifier;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::codec::from_bytes;
use continuumd::daemon::family::Arguments;
use continuumd::daemon::identity::{Blake3Identity, intent_to_wire};
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, ServerLimits, ServerWelcome,
    VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, ByteCount, CapabilityHandle, Commitment, DurationMs, EpochIdentity, IntentHandle,
    Opaque, OperationName, ProtocolVersion, RequestId, Timestamp,
};
use continuumd::protocol::shared::{FileComponent, SnapshotComponents, SnapshotEpochs};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{AuthorityLevel, Encoding, ResultStatus};
use continuumd::transport::{LocalPair, Server, encode_hello};

use crate::corpus::{CONTRACT, Source};

/// The protocol version every rig speaks.
pub const VERSION: (u32, u32) = (3, 2);

/// The one timestamp in this crate.
///
/// A benchmark that read a clock would produce a different artifact on every run, which is
/// the failure INV-005 exists to prevent and the reason `IMPL-05`'s two-fresh-runs device
/// can be a byte comparison at all.
pub const NOW: &str = "2026-08-01T00:00:00.000Z";

/// One principal: who is speaking, and under which capability.
///
/// Four are minted, at the four authority levels the benchmark needs. They are `&'static
/// str` rather than parsed handles because a policy names them as data, and the parse
/// happens once, where the envelope is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Principal {
    /// The actor identity.
    pub actor: &'static str,
    /// The capability handle.
    pub capability: &'static str,
}

impl Principal {
    /// `promote`, and the only principal that may accept an intent.
    pub const ROOT: Self = Self {
        actor: "service:continuumd",
        capability: "cap_root",
    };
    /// `propose`: creates and seals snapshots.
    pub const BUILDER: Self = Self {
        actor: "agent:builder",
        capability: "cap_builder",
    };
    /// `execute`: starts, resumes and cancels campaigns.
    pub const RUNNER: Self = Self {
        actor: "agent:runner",
        capability: "cap_runner",
    };
    /// `read`: reads task records and results, and nothing else. This is the principal the
    /// `WrongPrincipal` fault uses, because a read capability presenting itself for an
    /// `execute` operation is the most ordinary authority mistake an agent makes.
    pub const READER: Self = Self {
        actor: "agent:reader",
        capability: "cap_reader",
    };

    /// The parsed actor identity.
    ///
    /// # Panics
    ///
    /// Never: the four constants are well-formed, and this type has no other constructor
    /// reachable from outside this crate's own literals.
    #[must_use]
    pub fn actor_id(self) -> ActorId {
        ActorId::new(self.actor).expect("a well-formed actor identity")
    }

    /// The parsed capability handle.
    ///
    /// # Panics
    ///
    /// Never, for the same reason as [`Principal::actor_id`].
    #[must_use]
    pub fn capability_handle(self) -> CapabilityHandle {
        CapabilityHandle::new(self.capability).expect("a well-formed capability handle")
    }
}

/// What one corpus port's staged content is named by, once a daemon holds it.
#[derive(Debug, Clone)]
pub struct Staged {
    /// Content identities of every file, in path order.
    pub files: Vec<Commitment>,
    /// Where each file goes.
    pub placements: Vec<FileComponent>,
    /// Content identities of the configuration files, in path order.
    pub configuration: Vec<Commitment>,
}

/// A provisioned deployment: a daemon behind a byte boundary, plus what a request needs to
/// name a corpus port.
#[derive(Debug)]
pub struct Rig {
    server: Server,
    pair: LocalPair,
    hello_frame: Vec<u8>,
    welcome_frame: Vec<u8>,
    intent: IntentHandle,
    staged: BTreeMap<Source, Staged>,
}

impl Rig {
    /// Build a fresh deployment.
    ///
    /// # Panics
    ///
    /// When the fixture material is not what this crate ships — a corpus file that does not
    /// stage, an intent contract that does not decode, a handshake that does not reach 3.2.
    /// Every one of those is a broken build rather than a runtime condition, and a benchmark
    /// that continued past one would be measuring a daemon nobody provisioned.
    #[must_use]
    pub fn fresh() -> Self {
        let hello = hello();
        let hello_frame = encode_hello(&hello).expect("the hello frames");
        let negotiated = negotiate(&implemented(), ProtocolWindow::new(3), ENCODINGS, &hello)
            .expect("3.2 is served");

        let mut daemon = daemon(negotiated);
        let intent = accept_intent(&mut daemon);
        let staged = stage_corpus(&mut daemon);

        let welcome = welcome_for(negotiated);
        let welcome_frame = Server::open(&welcome, None, Ok(negotiated))
            .expect("the welcome encodes")
            .expect("a welcome frame is sent");
        let decoded: ServerWelcome = from_bytes(&welcome_frame).expect("the welcome decodes");
        assert_eq!(
            decoded, welcome,
            "the welcome crosses the boundary unchanged"
        );

        Self {
            server: Server::new(daemon, negotiated),
            pair: LocalPair::new(),
            hello_frame,
            welcome_frame,
            intent,
            staged,
        }
    }

    /// The negotiated protocol version.
    #[must_use]
    pub fn version(&self) -> ProtocolVersion {
        self.server.negotiated().protocol_version()
    }

    /// The `ClientHello` frame this deployment negotiated with.
    #[must_use]
    pub fn hello_frame(&self) -> &[u8] {
        &self.hello_frame
    }

    /// The `ServerWelcome` frame this deployment answers a hello with.
    #[must_use]
    pub fn welcome_frame(&self) -> &[u8] {
        &self.welcome_frame
    }

    /// The governing intent every snapshot references.
    #[must_use]
    pub const fn intent(&self) -> &IntentHandle {
        &self.intent
    }

    /// The daemon behind the boundary, for corroborating a wire fact one layer down.
    ///
    /// Used only where a number has no wire home — the labelled transition count and the
    /// witness depth, exactly as `continuumd`'s own PR-8 exit evidence reads them — and every
    /// use says so at the point it reads.
    #[must_use]
    pub const fn daemon(&self) -> &Daemon {
        self.server.daemon()
    }

    /// The three parts of the byte boundary, borrowed together.
    ///
    /// One method rather than three accessors because a caller needs the server and the pair
    /// mutably *at the same time* — which is what a link is — and Rust will only split those
    /// borrows through one function.
    // Not `const`: `Vec::as_slice` became const-callable in 1.87 and this workspace's
    // `rust-version` floor is 1.85 (`Cargo.toml`), which `clippy::incompatible_msrv`
    // enforces. Borrowing the whole vector avoids the call and keeps the floor honest.
    pub fn boundary(&mut self) -> (&mut Server, &mut LocalPair, &[u8]) {
        (&mut self.server, &mut self.pair, &self.welcome_frame)
    }

    /// The snapshot components naming one corpus port's staged content.
    ///
    /// # Panics
    ///
    /// When `source` was not staged, which cannot happen: [`Rig::fresh`] stages every member
    /// of [`Source::ALL`].
    #[must_use]
    pub fn components(&self, source: Source) -> SnapshotComponents {
        let staged = self.staged.get(&source).expect("every source is staged");
        SnapshotComponents {
            files: staged.files.clone(),
            cml_modules: Vec::new(),
            rust_extraction: Vec::new(),
            domain_packs: Vec::new(),
            dependencies: Vec::new(),
            epochs: SnapshotEpochs {
                semantic: epoch("semantic-1"),
                proof: epoch("proof-1"),
                toolchain: Optional::Absent,
            },
            intent: self.intent.clone(),
            correspondence: Vec::new(),
            proof_environment: Vec::new(),
            configuration: staged.configuration.clone(),
            file_components: Optional::Present(staged.placements.clone()),
        }
    }
}

/// The six epochs every request and result pins.
#[must_use]
pub fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Null,
        proof: Nullable::Value(epoch("proof-1")),
        corpus: Nullable::Value(epoch("tla-examples-1")),
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
}

/// The protocol version this benchmark speaks.
#[must_use]
pub fn version() -> ProtocolVersion {
    ProtocolVersion::new(VERSION.0, VERSION.1)
}

/// The client hello every arm opens with.
///
/// One hello for both arms: the handshake is part of what a surface costs, and two different
/// hellos would put a byte difference in the measurement that has nothing to do with the
/// interfaces being compared.
#[must_use]
pub fn hello() -> ClientHello {
    ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuum-benchmark".to_owned(),
        actor: Principal::ROOT.actor_id(),
        capability: Principal::ROOT.capability_handle(),
        features: Optional::Absent,
    }
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn implemented() -> Vec<ProtocolVersion> {
    vec![
        ProtocolVersion::new(2, 0),
        ProtocolVersion::new(3, 0),
        ProtocolVersion::new(3, 1),
        version(),
    ]
}

fn grant(principal: Principal, level: AuthorityLevel, depth: u32) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: principal.capability_handle(),
        actor: principal.actor_id(),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: depth,
        profile: Optional::Absent,
    }
}

fn root_grant() -> CapabilityDescriptor {
    CapabilityDescriptor {
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: vec![
                OperationName::new("intent.accept").expect("a registry name"),
            ],
            denied_operations: Vec::new(),
            data_grants: Vec::new(),
            cross_principal_sharing: true,
        }),
        ..grant(Principal::ROOT, AuthorityLevel::Promote, 4)
    }
}

fn welcome_for(negotiated: Negotiated) -> ServerWelcome {
    ServerWelcome {
        protocol_version: negotiated.protocol_version(),
        encoding: negotiated.encoding(),
        majors_served: vec![3, 2],
        server: "continuumd/0".to_owned(),
        grant: root_grant(),
        limits: ServerLimits {
            idempotency_retention_ms: DurationMs::new(86_400_000),
            max_page_size: 100,
            max_result_bytes: ByteCount::new(1_048_576),
            max_concurrent_tasks: 4,
        },
        features: Vec::new(),
        epochs: epochs(),
        pending_advances: Vec::new(),
    }
}

fn daemon(negotiated: Negotiated) -> Daemon {
    let root = Some(Principal::ROOT.capability_handle());
    Daemon::builder(
        Blake3Identity,
        negotiated,
        Principal::ROOT.capability_handle(),
    )
    .epochs(epochs())
    .now(Timestamp::new(NOW).expect("a timestamp"))
    .capability(root_grant(), None)
    .capability(
        grant(Principal::BUILDER, AuthorityLevel::Propose, 3),
        root.clone(),
    )
    .capability(
        grant(Principal::RUNNER, AuthorityLevel::Execute, 3),
        root.clone(),
    )
    .capability(grant(Principal::READER, AuthorityLevel::Read, 3), root)
    .family(WorkspaceFamily)
    .family(IntentFamily)
    .family(TaskFamily)
    .family(VerificationFamily)
    .build()
}

/// Register the governing contract and accept it, returning its handle.
fn accept_intent(daemon: &mut Daemon) -> IntentHandle {
    let contract =
        IntentContract::decode(CONTRACT.trim_end().as_bytes()).expect("the fixture decodes");
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    let intent = intent_to_wire(&stored).expect("an `in_` handle");
    daemon.state_mut().put_intent(
        intent.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );

    let accepted = daemon.dispatch(&OperationRequest {
        envelope: RequestEnvelope {
            protocol_version: version(),
            request_id: RequestId::new("req_accept").expect("a request id"),
            idempotency_key: Optional::Present("idem-accept".to_owned()),
            actor: Principal::ROOT.actor_id(),
            capability: Principal::ROOT.capability_handle(),
            operation: OperationName::new("intent.accept").expect("a registry name"),
            snapshot: Nullable::Null,
            intent: Nullable::Null,
            arguments: Opaque::from_bytes(Vec::new()),
            budget: Optional::Absent,
            output_policy: Optional::Absent,
            trace: Optional::Absent,
            page: Optional::Absent,
        },
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal: intent.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
    });
    assert_eq!(
        accepted.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        accepted.envelope.error
    );
    intent
}

fn acceptance_bytes() -> Opaque {
    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "service:continuumd"),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", NOW),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    Opaque::from_bytes(Json::Object(fields).to_canonical_bytes())
}

/// Stage every corpus port's files and register the model each resolves to.
fn stage_corpus(daemon: &mut Daemon) -> BTreeMap<Source, Staged> {
    let mut staged = BTreeMap::new();
    for source in Source::ALL {
        let mut files = Vec::new();
        let mut placements = Vec::new();
        let mut configuration = Vec::new();
        let module = (source.module_path(), source.module());
        for (path, content) in core::iter::once(module).chain(source.companions().iter().copied()) {
            let commitment = stage(daemon, path, content);
            placements.push(FileComponent {
                path: path.to_owned(),
                commitment: commitment.clone(),
            });
            files.push(commitment);
        }
        for (path, content) in source.configuration() {
            configuration.push(stage(daemon, path, content));
        }
        daemon.state_mut().models_mut().register(
            model_source(
                &Blake3Identity,
                [(source.module_path(), source.module().as_bytes())],
            )
            .expect("blake3 names the module set"),
            source.model().expect("the port builds"),
        );
        staged.insert(
            source,
            Staged {
                files,
                placements,
                configuration,
            },
        );
    }
    staged
}

fn stage(daemon: &mut Daemon, path: &str, content: &str) -> Commitment {
    daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new(path).expect("a workspace path"),
            content.as_bytes().to_vec(),
        )
        .expect("staging names its content")
}

/// The content identity seam this rig uses, exposed for a caller that needs to name content
/// the same way the daemon does.
#[must_use]
pub const fn identifier() -> &'static dyn ContentIdentifier {
    &Blake3Identity
}
