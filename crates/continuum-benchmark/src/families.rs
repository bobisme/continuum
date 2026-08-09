//! Fallible-policy families: research/25's mistake classes, injected on one shared policy,
//! measured per arm.
//!
//! # The question this module exists to answer
//!
//! bn-2c0a's campaign left the invalid-action margin at a **tie** — 69‰ on both arms — and
//! proved the tie is a property of the *policy* rather than of the two surfaces:
//!
//! > the shared policy can only make mistakes both surfaces can express; 0.6 shell-only
//! > invalid attempts per run clears the ratified 50% reduction. The tie is a lower bound on
//! > the typed arm's advantage, not a measurement of it.
//! >
//! > — `tests/dx10_falsification.rs`, surviving conclusion 2
//!
//! and named what was missing:
//!
//! > research/25's own mechanism for the invalid-action margin — validating a plan against
//! > the register before acting — is unexercised, and this campaign did not build it.
//! >
//! > — surviving conclusion 7
//!
//! This module builds it. A [`MistakeFamily`] is a **declared, seeded, deterministic** class
//! of agent mistakes injected into the shared policy. Both arms run the same policy with the
//! same seeds and receive the identical [`Mistake`] at the identical step; what differs is
//! whether the surface has a channel through which the mistake can be made at all. That
//! asymmetry is the measurement.
//!
//! # The fairness rule, restated for mistakes
//!
//! `crate::policy`'s load-bearing rule is that one policy drives both arms. A family does not
//! weaken it — it *extends* it to the argument level. A family names an **intent**: "the
//! agent means to invoke `verification.start` over this snapshot, and gets the third argument
//! wrong". Both arms are handed that intent. On the text surface the third argument is a
//! `--flag` and a name can be wrong; on the typed surface the third argument is a struct
//! field the compiler resolves, and there is no call through which a caller can offer a
//! different one. The intent is identical; the expressibility is not.
//!
//! # Three expressibilities, and why the third is not "an attempt"
//!
//! [`Expressibility`] is a closed set of three, and the distinction between the last two is
//! the one an adjudicator has to be able to check:
//!
//! - [`Expressibility::Wire`] — the surface carries the mistake to the daemon. One invalid
//!   attempt, one round trip.
//! - [`Expressibility::Local`] — the surface's own layer catches it before the wire. Still an
//!   attempt and still invalid — `crate::run`'s IMPL-01 section fixed that rule before any
//!   number was taken — but the cost is whatever that layer charged: zero for the typed
//!   client's register, the usage text for a CLI's argument parser.
//! - [`Expressibility::Unrepresentable`] — there is no channel. Nothing is composed, nothing
//!   is refused, and no artifact of the attempt exists anywhere. It is **not counted as an
//!   attempt**, because counting it would be charging for a counterfactual.
//!
//! The line between `Local` and `Unrepresentable` is auditable rather than asserted: a local
//! refusal produces a value (`continuum_mcp::register::Unmet`, or a CLI's usage text) and an
//! unrepresentable mistake produces none. `crate::run` counts the first and skips the second,
//! and the *other* reading — count every prevented intent as an invalid attempt on both arms
//! — is reported as a sensitivity row rather than argued away.
//!
//! # Anti-gaming: the containment theorem
//!
//! A family set built only from text-expressible mistakes would smuggle the answer. What
//! rules that out here is a structural fact about the two surfaces, stated as a theorem and
//! checked mechanically by [`typed_channels_are_contained_in_text_channels`]:
//!
//! > Every mistake channel the typed surface has, the text surface also has; the text surface
//! > has two the typed surface does not.
//!
//! The reason is that both arms build the **same typed request structs** — `crate::shell`'s
//! `arguments_for` is the one place a `Call` becomes an `Arguments`, and the typed client's
//! named methods build the identical structs. So any wrong *value* the typed client can pass,
//! a CLI can pass too, by writing it after the flag. What a CLI has in addition is the flag
//! itself: a string-keyed argument channel, with 25 positions ([`SITES`]), in which a name can
//! be wrong or missing. Every one of those 25 positions is listed here beside the
//! compiler-resolved field it corresponds to, so a reader can check the claim position by
//! position rather than take it.
//!
//! The consequence for family design is a rule this module obeys and states: **a class the
//! typed surface can express is injected on the typed arm too.** Three of the five families
//! below are exactly that, and they measure a margin of zero — which is the evidence that the
//! other two are not an artefact of the taxonomy.
//!
//! [`typed_channels_are_contained_in_text_channels`]: crate::families::typed_channels_are_contained_in_text_channels

use std::collections::BTreeMap;

use continuumd::protocol::scalar::{ContinuationHandle, TaskHandle, WorkspaceHandle};

use crate::policy::{AgentView, Call, Policy, PolicyKind, SCHEDULES, Step};
use crate::report::RATIFIED;
use crate::rig::{Principal, Rig};
use crate::run::{self, ArmRun};
use crate::shell::ShellSurface;
use crate::surface::{Arm, SurfaceError};
use crate::task::{self, BenchmarkTask, SUBSET};

// --- what a mistake is made *in* ------------------------------------------------------------

/// The part of a call a mistake lands in.
///
/// A closed set, and the unit the containment theorem is stated over. Two of the five are
/// properties of *how a call is written*; three are properties of *what the call says*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Channel {
    /// Which operation is invoked from the state the agent is in.
    OperationChoice,
    /// Which capability the request presents.
    Capability,
    /// Which handle value an argument carries.
    HandleValue,
    /// Whether an argument is written at all.
    ArgumentPresence,
    /// What an argument is *called*.
    ArgumentName,
}

impl Channel {
    /// Every channel, in report order.
    pub const ALL: [Self; 5] = [
        Self::OperationChoice,
        Self::Capability,
        Self::HandleValue,
        Self::ArgumentPresence,
        Self::ArgumentName,
    ];

    /// The stable token a report writes this channel as.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::OperationChoice => "operation-choice",
            Self::Capability => "capability",
            Self::HandleValue => "handle-value",
            Self::ArgumentPresence => "argument-presence",
            Self::ArgumentName => "argument-name",
        }
    }

    /// Whether `arm`'s surface has this channel at all.
    ///
    /// The typed client reaches every operation through a method whose parameters are typed
    /// values, so `ArgumentName` and `ArgumentPresence` do not exist on it: a field name is
    /// resolved by the compiler and a missing parameter is a compile error, not a call. The
    /// other three exist on both arms because both arms build the same request structs — see
    /// this module's containment theorem.
    #[must_use]
    pub const fn exists_on(self, arm: Arm) -> bool {
        !matches!(
            (self, arm),
            (Self::ArgumentName | Self::ArgumentPresence, Arm::Native)
        )
    }
}

// --- what a surface does with a mistake -----------------------------------------------------

/// What happens when a policy hands one surface one mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Expressibility {
    /// The surface carries it to the daemon: one invalid attempt, one round trip.
    Wire,
    /// The surface's own layer refuses it before the wire: one invalid attempt, whatever that
    /// layer charged.
    Local,
    /// There is no channel: nothing is composed, nothing is refused, nothing is attempted.
    Unrepresentable,
}

impl Expressibility {
    /// The stable token a report writes this expressibility as.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Wire => "wire",
            Self::Local => "local",
            Self::Unrepresentable => "unrepresentable",
        }
    }

    /// Whether the mistake is attempted at all on this surface.
    #[must_use]
    pub const fn attempted(self) -> bool {
        !matches!(self, Self::Unrepresentable)
    }
}

// --- the mistake classes ---------------------------------------------------------------------

/// One class of agent mistake.
///
/// Five, and the set is not a free choice: it is [`Channel::ALL`] with the two argument
/// channels split into "wrong name" and "no name at all", which are different mistakes with
/// different CLI diagnostics. Every channel is covered, so the taxonomy cannot have omitted a
/// typed-expressible class — that is what [`Channel::ALL`] being exhausted here means, and
/// `tests/pr10_fallible_families.rs` asserts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MistakeClass {
    /// Invoke an operation whose precondition does not hold: `verification.start` over a
    /// snapshot that was never sealed.
    OutOfGrammar,
    /// Name a snapshot handle a later operation superseded.
    StaleSnapshot,
    /// Leave a required argument out of the call entirely.
    OmittedArgument,
    /// Present a capability the operation does not accept: `read` where `execute` is needed.
    WrongCapability,
    /// Write a required argument under a name the surface does not know.
    MisnamedArgument,
}

impl MistakeClass {
    /// Every class, in shortlex order of [`MistakeClass::token`].
    pub const ALL: [Self; 5] = [
        Self::OutOfGrammar,
        Self::StaleSnapshot,
        Self::OmittedArgument,
        Self::WrongCapability,
        Self::MisnamedArgument,
    ];

    /// The stable token a report writes this class as.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::OutOfGrammar => "out-of-grammar",
            Self::StaleSnapshot => "stale-snapshot",
            Self::OmittedArgument => "omitted-argument",
            Self::WrongCapability => "wrong-capability",
            Self::MisnamedArgument => "misnamed-argument",
        }
    }

    /// The channel this class is a mistake in.
    #[must_use]
    pub const fn channel(self) -> Channel {
        match self {
            Self::OutOfGrammar => Channel::OperationChoice,
            Self::StaleSnapshot => Channel::HandleValue,
            Self::OmittedArgument => Channel::ArgumentPresence,
            Self::WrongCapability => Channel::Capability,
            Self::MisnamedArgument => Channel::ArgumentName,
        }
    }

    /// What `arm` does with this class.
    ///
    /// Declared here and **checked against what the arms actually did** by
    /// `tests/pr10_fallible_families.rs`: a class declared `Unrepresentable` on an arm that
    /// nonetheless spent a byte, or declared `Wire` on an arm that never reached the daemon,
    /// fails that test. The table is a claim this crate can be caught making wrongly.
    #[must_use]
    pub const fn expressibility(self, arm: Arm) -> Expressibility {
        if !self.channel().exists_on(arm) {
            return Expressibility::Unrepresentable;
        }
        match (self, arm) {
            // The typed client's register is research/25's semantic action grammar, and it
            // refuses an operation whose precondition does not hold before writing a frame.
            // A CLI has no such table: it writes the command and the daemon answers.
            (Self::OutOfGrammar, Arm::Native) => Expressibility::Local,
            _ => Expressibility::Wire,
        }
    }

    /// Whether the two arms differ on this class at all.
    ///
    /// True of three classes, and only two of them for the same reason:
    /// `misnamed-argument` and `omitted-argument` because the typed surface has no channel,
    /// and `out-of-grammar` because it has the *same* channel and a grammar that refuses it
    /// before the wire. The first two move the invalid-action metric; the third moves only
    /// bytes. [`MistakeClass::prevented_on_typed`] is the predicate that separates them.
    #[must_use]
    pub fn asymmetric(self) -> bool {
        self.expressibility(Arm::Native) != self.expressibility(Arm::Shell)
    }

    /// Whether the typed surface has no channel for this class at all.
    ///
    /// The predicate the headline partitions on: only a class the typed surface cannot
    /// express produces a **shell-only invalid attempt**, which is the quantity plan §24.5's
    /// invalid-action margin — and bn-2c0a's 0.6 break-even — is stated in.
    #[must_use]
    pub const fn prevented_on_typed(self) -> bool {
        matches!(
            self.expressibility(Arm::Native),
            Expressibility::Unrepresentable
        )
    }
}

/// The containment theorem, as a decidable predicate.
///
/// Every channel the typed surface has, the text surface has; the text surface has at least
/// one the typed surface does not. `false` would mean a mistake class exists that only the
/// typed arm can make, and that the families here omitted it.
#[must_use]
pub fn typed_channels_are_contained_in_text_channels() -> bool {
    let contained = Channel::ALL
        .iter()
        .all(|channel| !channel.exists_on(Arm::Native) || channel.exists_on(Arm::Shell));
    let strict = Channel::ALL
        .iter()
        .any(|channel| channel.exists_on(Arm::Shell) && !channel.exists_on(Arm::Native));
    contained && strict
}

// --- one injected mistake ----------------------------------------------------------------------

/// One mistake, attached to the step the policy decided to make.
///
/// The two argument mistakes name a *position* rather than a flag, because the position is
/// what both surfaces have in common: argument 2 of `verification.start` is `--states` on one
/// surface and `Budget.states` on the other, and the family names the position so the intent
/// is identical on both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mistake {
    /// Write argument `ordinal` under a name the surface does not know.
    MisnamedArgument {
        /// Which argument position, zero-based, in the order the command writes them.
        ordinal: u32,
    },
    /// Leave argument `ordinal` out of the call.
    OmittedArgument {
        /// Which argument position, zero-based.
        ordinal: u32,
    },
    /// A mistake already carried by the call itself: a wrong operation, handle or capability.
    ///
    /// Both arms make it identically, because both build the same request from the same
    /// [`Call`]. It is recorded so the report can attribute the attempt to a class.
    InCall(MistakeClass),
}

impl Mistake {
    /// The class this mistake belongs to.
    #[must_use]
    pub const fn class(self) -> MistakeClass {
        match self {
            Self::MisnamedArgument { .. } => MistakeClass::MisnamedArgument,
            Self::OmittedArgument { .. } => MistakeClass::OmittedArgument,
            Self::InCall(class) => class,
        }
    }

    /// Whether this mistake is made in how the call is *written* rather than in what it says.
    ///
    /// The written ones are the two the surfaces disagree about, and the only ones a
    /// [`Surface`](crate::surface::Surface) has to handle: everything else is already in the
    /// [`Call`] by the time an arm sees it.
    #[must_use]
    pub const fn written(self) -> bool {
        matches!(
            self,
            Self::MisnamedArgument { .. } | Self::OmittedArgument { .. }
        )
    }

    /// The stable token a transcript writes this mistake as.
    #[must_use]
    pub fn token(self) -> String {
        match self {
            Self::MisnamedArgument { ordinal } => format!("misnamed-argument@{ordinal}"),
            Self::OmittedArgument { ordinal } => format!("omitted-argument@{ordinal}"),
            Self::InCall(class) => class.token().to_owned(),
        }
    }
}

// --- the misnameable argument positions ---------------------------------------------------------

/// One place a text surface names an argument, and the typed counterpart that has no name.
///
/// `typed` is the whole anti-gaming argument made position by position: every one of the 25
/// is a struct field or an envelope field that the compiler resolves, so there is no call
/// through which a typed caller could offer a different name for it or leave it out. A reader
/// who doubts the claim can check it here rather than take it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Site {
    /// The wire operation whose command line writes this argument.
    pub operation: &'static str,
    /// Which argument position, zero-based, in the order the command writes them.
    pub ordinal: u32,
    /// The flag a CLI names it by.
    pub flag: &'static str,
    /// The compiler-resolved field the typed client passes instead.
    pub typed: &'static str,
    /// Whether every solved run in the matrix issues this operation.
    ///
    /// The five operations a completed run cannot avoid — create, seal, start, status,
    /// result — carry [`GUARANTEED`] of the 25 positions. A family that injected at a
    /// position no run visits would report a rate below its own declared one for a reason
    /// that has nothing to do with either surface, so the rate-bearing families draw only
    /// from these.
    pub guaranteed: bool,
}

/// Every place the text surface names an argument: 25 positions over 8 operations.
///
/// Derived from `crate::shell::command_line` rather than invented, and
/// `tests/pr10_fallible_families.rs` re-derives it from that function and compares.
pub const SITES: &[Site] = &[
    Site {
        operation: "workspace.create",
        ordinal: 0,
        flag: "--port",
        typed: "WorkspaceCreateRequest.components",
        guaranteed: true,
    },
    Site {
        operation: "workspace.create",
        ordinal: 1,
        flag: "--seal",
        typed: "WorkspaceCreateRequest.seal",
        guaranteed: true,
    },
    Site {
        operation: "workspace.create",
        ordinal: 2,
        flag: "--as",
        typed: "RequestEnvelope.actor+capability",
        guaranteed: true,
    },
    Site {
        operation: "workspace.fork",
        ordinal: 0,
        flag: "--base",
        typed: "WorkspaceForkRequest.base",
        guaranteed: false,
    },
    Site {
        operation: "workspace.fork",
        ordinal: 1,
        flag: "--set",
        typed: "WorkspaceForkRequest.overlay",
        guaranteed: false,
    },
    Site {
        operation: "workspace.fork",
        ordinal: 2,
        flag: "--as",
        typed: "RequestEnvelope.actor+capability",
        guaranteed: false,
    },
    Site {
        operation: "workspace.seal",
        ordinal: 0,
        flag: "--snapshot",
        typed: "WorkspaceSealRequest.snapshot",
        guaranteed: true,
    },
    Site {
        operation: "workspace.seal",
        ordinal: 1,
        flag: "--as",
        typed: "RequestEnvelope.actor+capability",
        guaranteed: true,
    },
    Site {
        operation: "verification.start",
        ordinal: 0,
        flag: "--snapshot",
        typed: "RequestEnvelope.snapshot",
        guaranteed: true,
    },
    Site {
        operation: "verification.start",
        ordinal: 1,
        flag: "--target",
        typed: "VerificationStartRequest.target",
        guaranteed: true,
    },
    Site {
        operation: "verification.start",
        ordinal: 2,
        flag: "--states",
        typed: "Budget.states",
        guaranteed: true,
    },
    Site {
        operation: "verification.start",
        ordinal: 3,
        flag: "--wall-ms",
        typed: "Budget.wall_ms",
        guaranteed: true,
    },
    Site {
        operation: "verification.start",
        ordinal: 4,
        flag: "--bytes",
        typed: "Budget.bytes",
        guaranteed: true,
    },
    Site {
        operation: "verification.start",
        ordinal: 5,
        flag: "--as",
        typed: "RequestEnvelope.actor+capability",
        guaranteed: true,
    },
    Site {
        operation: "task.status",
        ordinal: 0,
        flag: "--task",
        typed: "TaskStatusRequest.task",
        guaranteed: true,
    },
    Site {
        operation: "task.status",
        ordinal: 1,
        flag: "--as",
        typed: "RequestEnvelope.actor+capability",
        guaranteed: true,
    },
    Site {
        operation: "verification.result",
        ordinal: 0,
        flag: "--task",
        typed: "VerificationResultRequest.task",
        guaranteed: true,
    },
    Site {
        operation: "verification.result",
        ordinal: 1,
        flag: "--as",
        typed: "RequestEnvelope.actor+capability",
        guaranteed: true,
    },
    Site {
        operation: "task.resume",
        ordinal: 0,
        flag: "--continuation",
        typed: "TaskResumeRequest.continuation",
        guaranteed: false,
    },
    Site {
        operation: "task.resume",
        ordinal: 1,
        flag: "--states",
        typed: "Budget.states",
        guaranteed: false,
    },
    Site {
        operation: "task.resume",
        ordinal: 2,
        flag: "--wall-ms",
        typed: "Budget.wall_ms",
        guaranteed: false,
    },
    Site {
        operation: "task.resume",
        ordinal: 3,
        flag: "--bytes",
        typed: "Budget.bytes",
        guaranteed: false,
    },
    Site {
        operation: "task.resume",
        ordinal: 4,
        flag: "--as",
        typed: "RequestEnvelope.actor+capability",
        guaranteed: false,
    },
    Site {
        operation: "task.cancel",
        ordinal: 0,
        flag: "--task",
        typed: "TaskCancelRequest.task",
        guaranteed: false,
    },
    Site {
        operation: "task.cancel",
        ordinal: 1,
        flag: "--as",
        typed: "RequestEnvelope.actor+capability",
        guaranteed: false,
    },
];

/// How many of [`SITES`] lie on an operation every solved run issues.
pub const GUARANTEED: usize = 15;

/// The state ceiling [`MistakeClass::OutOfGrammar`]'s mistaken `verification.start` declares.
///
/// A dedicated constant, distinct from every task's [`BenchmarkTask::ceiling`] and from
/// `crate::policy::TIGHT_CEILING`, and the reason is an instrument hazard worth recording
/// rather than working around silently.
///
/// The step is refused on a *precondition* — the snapshot was never sealed — so the ceiling it
/// declares cannot affect the outcome. What it does affect is the **command text**, and
/// `crate::shell::dispatch` derives a CLI process's idempotency key from exactly that text
/// ("a CLI process derives a request identifier from the command it is running, which is what
/// makes a shell transcript reproducible"). A mistaken start that declared the same ceiling as
/// the legitimate one that follows it would therefore write the *identical* command, spend the
/// *identical* idempotency key, and be answered by the daemon's replay of its own earlier
/// refusal — for every retry, until the run exhausted its attempts. Measured, that is a
/// baseline that solves 4 of 24 tasks and an invalid-action rate of 931‰.
///
/// That collapse is real, but it prices **this harness's key derivation**, not the interface,
/// and banking it would be exactly the kind of advantage bn-2c0a's discipline exists to
/// refuse. So the mistaken command is made distinguishable, the family measures a margin of
/// zero on the invalid-action metric, and the hazard is reported as an attack rather than as a
/// result.
///
/// [`BenchmarkTask::ceiling`]: crate::task::BenchmarkTask::ceiling
pub const OUT_OF_GRAMMAR_CEILING: u64 = 1;

/// The [`SITES`] entries every solved run reaches, in table order.
#[must_use]
pub fn guaranteed_sites() -> Vec<&'static Site> {
    SITES.iter().filter(|site| site.guaranteed).collect()
}

// --- families -------------------------------------------------------------------------------------

/// One fallible-policy family: a class of mistake, injected once per run.
///
/// The **rate is one mistake per run, exactly**, and that is a floor rather than a tuned
/// value: it is the smallest non-zero rate a per-run injection can have. A family that fired
/// twice would report a larger margin and would be answering a question nobody asked; a
/// family that fired less than once per run would need a probability, and this instrument has
/// no way to justify one (INV-005 keeps the entropy out, and `crate::falsification`'s limits
/// already say a scripted policy cannot have a rate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MistakeFamily {
    /// The stable identifier a report names this family by.
    pub id: &'static str,
    /// The class of mistake it injects.
    pub class: MistakeClass,
    /// The ceiling an out-of-grammar `verification.start` declares.
    ///
    /// [`Some`] on every family in [`FAMILIES`], and [`None`] only on [`COLLIDING`], the
    /// attack family that deliberately writes the command a legitimate start would write.
    /// See [`OUT_OF_GRAMMAR_CEILING`] for what that collision costs and why banking it would
    /// be pricing this harness rather than the interface.
    pub ceiling: Option<u64>,
}

/// The control: the same matrix with no mistake injected.
///
/// Its runs must be the landed runs, byte for byte. That is what makes the family machinery
/// provably inert when no family is selected, and it is asserted rather than assumed.
pub const CONTROL: MistakeFamily = MistakeFamily {
    id: "mf-00-control",
    class: MistakeClass::OutOfGrammar,
    ceiling: Some(OUT_OF_GRAMMAR_CEILING),
};

/// The attack family: an out-of-grammar start that writes a legitimate command.
///
/// Not in [`FAMILIES`] and never part of a reported margin. It exists so the idempotency-key
/// hazard [`OUT_OF_GRAMMAR_CEILING`] describes can be *measured* rather than asserted, and
/// then declined.
pub const COLLIDING: MistakeFamily = MistakeFamily {
    id: "mf-x1-colliding-command",
    class: MistakeClass::OutOfGrammar,
    ceiling: None,
};

/// The five families, one per [`MistakeClass`], in report order.
///
/// Two are asymmetric (the argument channels the typed surface does not have) and three are
/// symmetric (the channels both surfaces have, injected on both arms). The three symmetric
/// ones are not filler: they are the measurement that the two asymmetric ones are not an
/// artefact of the taxonomy.
pub const FAMILIES: &[MistakeFamily] = &[
    MistakeFamily {
        id: "mf-01-out-of-grammar",
        class: MistakeClass::OutOfGrammar,
        ceiling: Some(OUT_OF_GRAMMAR_CEILING),
    },
    MistakeFamily {
        id: "mf-02-stale-snapshot",
        class: MistakeClass::StaleSnapshot,
        ceiling: Some(OUT_OF_GRAMMAR_CEILING),
    },
    MistakeFamily {
        id: "mf-03-omitted-argument",
        class: MistakeClass::OmittedArgument,
        ceiling: Some(OUT_OF_GRAMMAR_CEILING),
    },
    MistakeFamily {
        id: "mf-04-wrong-capability",
        class: MistakeClass::WrongCapability,
        ceiling: Some(OUT_OF_GRAMMAR_CEILING),
    },
    MistakeFamily {
        id: "mf-05-misnamed-argument",
        class: MistakeClass::MisnamedArgument,
        ceiling: Some(OUT_OF_GRAMMAR_CEILING),
    },
];

/// How many cells one family's sweep runs, per arm.
pub const CELLS: u32 = 24;

/// One family, bound to one cell of the matrix.
///
/// `cell` is the seed of the site choice, and it is an **explicit input** rather than a draw:
/// [`cells`] enumerates the matrix in one canonical order and hands each cell its index, so
/// cell 7 means the same thing on every machine, forever (INV-005).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Injection {
    /// The family being injected.
    pub family: MistakeFamily,
    /// Which cell of the matrix this is.
    pub cell: u32,
}

impl Injection {
    /// The site this cell's argument mistake lands on.
    ///
    /// The declared rule: `guaranteed_sites()[cell % GUARANTEED]`. With 24 cells over 15
    /// guaranteed sites every site is exercised at least once, and no site is chosen because
    /// of what it measures.
    ///
    /// # Panics
    ///
    /// Never: [`guaranteed_sites`] is non-empty and the index is taken modulo its length.
    #[must_use]
    pub fn site(self) -> &'static Site {
        let sites = guaranteed_sites();
        sites[self.cell as usize % sites.len()]
    }

    /// Turn the step the policy decided on into the family's mistake, once per run.
    ///
    /// Applied **after** `crate::policy`'s own fault perturbation, and it stands aside from
    /// any step that already carries a [`Fault`](crate::policy::Fault). The two injections
    /// have different owners — the four faults are IMPL-04's landed schedule — and composing
    /// them on one step would make a family's effect a function of the fault schedule: on the
    /// arm that cannot express the mistake the step is never attempted, so the *fault* on it
    /// is never attempted either, and the family would be measuring the erasure of somebody
    /// else's injection.
    ///
    /// The cost of standing aside is honest and reported: in a cell where every step of the
    /// right shape carries a fault, the family fires nothing, and the measured
    /// mistakes-per-run falls below the declared one rather than the family reaching for a
    /// step it should not have.
    #[must_use]
    pub fn apply(self, task: &BenchmarkTask, view: &AgentView, step: Step) -> Step {
        if step.fault.is_some() {
            return step;
        }
        let class = self.family.class;
        let made = view.mistakes.iter().filter(|made| **made == class).count();
        match class {
            MistakeClass::MisnamedArgument | MistakeClass::OmittedArgument => {
                let site = self.site();
                if made > 0 || step.call.operation() != site.operation {
                    return step;
                }
                let mistake = if class == MistakeClass::MisnamedArgument {
                    Mistake::MisnamedArgument {
                        ordinal: site.ordinal,
                    }
                } else {
                    Mistake::OmittedArgument {
                        ordinal: site.ordinal,
                    }
                };
                Step {
                    mistake: Some(mistake),
                    ..step
                }
            }
            MistakeClass::WrongCapability => {
                if made > 0 || !matches!(step.call, Call::StartVerification { .. }) {
                    return step;
                }
                Step {
                    principal: Principal::READER,
                    mistake: Some(Mistake::InCall(class)),
                    ..step
                }
            }
            MistakeClass::OutOfGrammar => {
                let Call::SealWorkspace { snapshot } = &step.call else {
                    return step;
                };
                if made > 0 {
                    return step;
                }
                Step {
                    call: Call::StartVerification {
                        snapshot: snapshot.clone(),
                        states: self.family.ceiling.unwrap_or(task.ceiling),
                    },
                    principal: Principal::RUNNER,
                    fault: step.fault,
                    mistake: Some(Mistake::InCall(class)),
                }
            }
            // Two steps, because a superseded handle has to be *made* superseded first and
            // the agent's own history is the only honest way to do it. The first occurrence
            // advances the lineage with an ordinary fork — admitted, symmetric, and no
            // mistake in itself. The second names the pre-fork snapshot the agent is still
            // holding, which is exactly the `StaleSnapshot` the daemon exists to answer
            // (`crate::policy::Call::RestoreModule` explains the lineage rule).
            MistakeClass::StaleSnapshot => {
                let Call::SealWorkspace { snapshot } = &step.call else {
                    return step;
                };
                match made {
                    0 => Step {
                        call: Call::ForkModule {
                            base: snapshot.clone(),
                        },
                        principal: Principal::BUILDER,
                        fault: step.fault,
                        mistake: Some(Mistake::InCall(class)),
                    },
                    1 => match view.origin.clone() {
                        Some(origin) if Some(&origin) != view.snapshot.as_ref() => Step {
                            call: Call::RestoreModule { base: origin },
                            principal: Principal::BUILDER,
                            fault: step.fault,
                            mistake: Some(Mistake::InCall(class)),
                        },
                        _ => step,
                    },
                    _ => step,
                }
            }
        }
    }
}

/// The matrix, in one canonical order, with each cell's index.
///
/// The same `task × seed × policy` matrix `crate::run::sweep` drives, enumerated once so both
/// arms' family sweeps agree on which cell is which without either arm deciding.
#[must_use]
pub fn cells() -> Vec<(&'static BenchmarkTask, u16, PolicyKind, u32)> {
    let mut out = Vec::new();
    let mut index = 0;
    for task in SUBSET {
        for schedule in SCHEDULES {
            for kind in PolicyKind::ALL {
                out.push((task, schedule.seed, kind, index));
                index += 1;
            }
        }
    }
    out
}

/// The policy one cell of a family's sweep runs.
#[must_use]
pub fn policy_for(family: MistakeFamily, seed: u16, kind: PolicyKind, cell: u32) -> Policy {
    if family == CONTROL {
        return Policy::new(kind, seed);
    }
    Policy::injecting(kind, seed, Injection { family, cell })
}

/// Drive the shell baseline over one family's whole matrix.
///
/// A fresh rig per cell, exactly as `crate::run::sweep` does, and the landed
/// [`ShellSurface::new`] baseline — nothing about the *surface* changes for a family sweep,
/// only what the policy decided to do.
///
/// # Errors
///
/// [`SurfaceError`] as `crate::run::drive`.
pub fn shell_sweep(family: MistakeFamily) -> Result<Vec<ArmRun>, SurfaceError> {
    let mut runs = Vec::new();
    for (task, seed, kind, cell) in cells() {
        let mut rig = Rig::fresh();
        let mut surface = ShellSurface::new();
        runs.push(run::drive(
            &mut surface,
            &mut rig,
            task,
            policy_for(family, seed, kind, cell),
        )?);
    }
    Ok(runs)
}

// --- what a text surface writes when it gets an argument wrong ---------------------------------

/// The command a mistaken agent wrote, and what the tool printed back.
///
/// No daemon sees either: a CLI refuses an unknown or missing argument in its own argument
/// parser, exactly as the typed client's register refuses an operation whose preconditions do
/// not hold. The two local refusals are the fair comparison; the difference is that one arm
/// *has* the mistake available to make.
#[must_use]
pub fn mistaken_command(command: &str, mistake: Mistake) -> (String, String) {
    let flags = crate::variants::flag_names(command);
    let ordinal = match mistake {
        Mistake::MisnamedArgument { ordinal } | Mistake::OmittedArgument { ordinal } => ordinal,
        Mistake::InCall(_) => return (command.to_owned(), String::new()),
    };
    let Some(meant) = flags.get(ordinal as usize).copied() else {
        return (command.to_owned(), String::new());
    };
    match mistake {
        Mistake::MisnamedArgument { .. } => {
            let offered = format!("--{}", &meant[3..]);
            (
                rewrite_flag(command, ordinal, Some(offered.as_str())),
                crate::variants::unknown_argument_usage(command, &offered, meant),
            )
        }
        Mistake::OmittedArgument { .. } => (
            rewrite_flag(command, ordinal, None),
            missing_argument_usage(command, meant),
        ),
        Mistake::InCall(_) => (command.to_owned(), String::new()),
    }
}

/// Rewrite the `ordinal`th flag: rename it, or drop it and the value it carries.
fn rewrite_flag(command: &str, ordinal: u32, renamed: Option<&str>) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut seen = 0;
    let mut drop_value = false;
    for token in command.split_whitespace() {
        if drop_value {
            drop_value = false;
            continue;
        }
        if token.starts_with("--") {
            if seen == ordinal {
                match renamed {
                    Some(name) => out.push(name.to_owned()),
                    None => drop_value = true,
                }
                seen += 1;
                continue;
            }
            seen += 1;
        }
        out.push(token.to_owned());
    }
    out.join(" ")
}

/// What a CLI prints when a required argument is not written at all.
///
/// Derived from the command like [`crate::variants::unknown_argument_usage`], and in the same
/// shape, so the two argument mistakes are priced by one convention rather than two.
#[must_use]
pub fn missing_argument_usage(command: &str, missing: &str) -> String {
    let usage = command
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "error: the following required arguments were not provided:\n  {missing} \
         <VALUE>\n\nUsage: {usage} {missing} <VALUE>\n\nFor more information, try \
         '--help'.\n"
    )
}

/// The transcript line one mistaken attempt writes.
///
/// The landed line plus what the mistake was and whether the surface could express it, so the
/// transcript is a full operation trace of the mistakes too (research/33, benchmark principle
/// 4) and an independent grader can recompute every count in this module's report from it.
#[must_use]
pub fn mistake_line(step: &Step, mistake: Mistake, expressed: bool, bytes: u64) -> String {
    format!(
        "{} as={} mistake={} expressed={} bytes={}",
        step.call.operation(),
        step.principal.actor,
        mistake.token(),
        expressed,
        bytes,
    )
}

// --- the report ---------------------------------------------------------------------------------

/// One arm's totals over one family's runs, restricted to one split.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FamilyTotals {
    /// Cells counted.
    pub runs: u32,
    /// Cells solved.
    pub solved: u32,
    /// Operations attempted.
    pub attempted: u32,
    /// Attempts that were not admitted.
    pub invalid: u32,
    /// Invalid attempts that spent no interface bytes.
    pub zero_cost_invalid: u32,
    /// Mistakes the policy decided on that this surface had no channel for.
    pub prevented: u32,
    /// Mistakes the policy decided on at all, expressible or not.
    ///
    /// Identical on both arms by construction — the policy is shared — so a difference
    /// between the two arms' other counters is a difference in expressibility and never in
    /// what the agent tried to do. It is also how many of the family's cells actually landed
    /// a mistake, which can be fewer than one per cell: see [`Injection::apply`].
    pub mistakes: u32,
    /// Interface bytes over every cell.
    pub bytes_total: u64,
    /// Interface bytes over the cells that were solved.
    pub bytes_on_solved: u64,
}

impl FamilyTotals {
    /// Fold one run in.
    pub fn absorb(&mut self, run: &ArmRun) {
        self.runs += 1;
        self.attempted += run.attempted;
        self.invalid += run.invalid;
        self.zero_cost_invalid += run.zero_cost_invalid;
        self.prevented += run.prevented;
        self.mistakes += u32::try_from(run.mistakes.len()).unwrap_or(u32::MAX);
        self.bytes_total += run.bytes;
        if run.solved {
            self.solved += 1;
            self.bytes_on_solved += run.bytes;
        }
    }

    /// Invalid-action rate, in per-mille of attempts.
    #[must_use]
    pub const fn invalid_permille(&self) -> i64 {
        if self.attempted == 0 {
            return 0;
        }
        (self.invalid as i64 * 1000) / self.attempted as i64
    }

    /// Task success, in per-mille.
    #[must_use]
    pub const fn success_permille(&self) -> i64 {
        if self.runs == 0 {
            return 0;
        }
        (self.solved as i64 * 1000) / self.runs as i64
    }

    /// Interface bytes per solved task, or [`None`] when nothing was solved.
    #[must_use]
    pub const fn bytes_per_solved(&self) -> Option<u64> {
        if self.solved == 0 {
            return None;
        }
        Some(self.bytes_on_solved / self.solved as u64)
    }
}

/// A family-level split of the matrix (research/33, benchmark principle 6).
///
/// A margin that held only on the development partition, or only on one semantic family,
/// would be an overfit to the tasks the harness was written against. Every family is reported
/// over all four splits, and the report is where a reader checks that the headline is not
/// carried by one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Split {
    /// Every task.
    All,
    /// One plan §19.2 semantic family.
    TaskFamily(task::Family),
    /// One plan §19.4 partition.
    Partition(task::Partition),
}

impl Split {
    /// Every split, in report order.
    #[must_use]
    pub fn all() -> Vec<Self> {
        let mut out = vec![Self::All];
        out.extend(task::Family::ALL.map(Self::TaskFamily));
        out.extend(task::Partition::ALL.map(Self::Partition));
        out
    }

    /// The stable token a report writes this split as.
    #[must_use]
    pub fn token(self) -> String {
        match self {
            Self::All => "all".to_owned(),
            Self::TaskFamily(family) => format!("family:{}", family.token()),
            Self::Partition(partition) => format!("partition:{}", partition.token()),
        }
    }

    /// Whether the task named by `id` is inside this split.
    #[must_use]
    pub fn holds(self, id: &str) -> bool {
        let Some(task) = SUBSET.iter().find(|task| task.id == id) else {
            return false;
        };
        match self {
            Self::All => true,
            Self::TaskFamily(family) => task.family == family,
            Self::Partition(partition) => task.partition == partition,
        }
    }
}

/// One family's margin on one split: both arms' totals and what separates them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyMargin {
    /// The family.
    pub family: &'static str,
    /// Its mistake class.
    pub class: MistakeClass,
    /// The split these totals cover.
    pub split: Split,
    /// The typed arm.
    pub native: FamilyTotals,
    /// The baseline.
    pub shell: FamilyTotals,
}

impl FamilyMargin {
    /// Fold both arms' runs into one family-and-split margin.
    #[must_use]
    pub fn measure(
        family: MistakeFamily,
        split: Split,
        native: &[ArmRun],
        shell: &[ArmRun],
    ) -> Self {
        let mut margin = Self {
            family: family.id,
            class: family.class,
            split,
            native: FamilyTotals::default(),
            shell: FamilyTotals::default(),
        };
        for run in native.iter().filter(|run| split.holds(run.task)) {
            margin.native.absorb(run);
        }
        for run in shell.iter().filter(|run| split.holds(run.task)) {
            margin.shell.absorb(run);
        }
        margin
    }

    /// Percent relative reduction in invalid-action rate, or [`None`] when the baseline's
    /// rate is zero and a relative reduction is undefined.
    #[must_use]
    pub fn reduction_percent(&self) -> Option<i64> {
        let shell = self.shell.invalid_permille();
        (shell > 0).then(|| ((shell - self.native.invalid_permille()) * 100) / shell)
    }

    /// Whether the ratified 50% relative reduction clears on this split.
    #[must_use]
    pub fn clears(&self) -> bool {
        self.reduction_percent()
            .is_some_and(|value| value >= RATIFIED.invalid_reduction_percent)
    }

    /// Shell-only invalid attempts per run, in hundredths.
    ///
    /// The quantity the break-even is stated in: how many invalid attempts the baseline makes
    /// that the typed arm does not. Hundredths rather than a float, because this artifact is
    /// compared byte for byte and a float's rendering is a portability question nobody should
    /// have to answer to read a benchmark.
    #[must_use]
    pub const fn shell_only_per_run_hundredths(&self) -> i64 {
        if self.shell.runs == 0 {
            return 0;
        }
        ((self.shell.invalid as i64 - self.native.invalid as i64) * 100) / self.shell.runs as i64
    }

    /// The margin, as one stable line.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "{} {} {} mistakes={} native_runs={} native_attempted={} native_invalid={} \
             native_invalid_permille={} native_zero_cost_invalid={} native_prevented={} \
             native_solved={} native_bytes_per_solved={} shell_runs={} shell_attempted={} \
             shell_invalid={} shell_invalid_permille={} shell_solved={} \
             shell_bytes_per_solved={} reduction_percent={} clears={} \
             shell_only_per_run_hundredths={}",
            self.family,
            self.class.token(),
            self.split.token(),
            self.shell.mistakes,
            self.native.runs,
            self.native.attempted,
            self.native.invalid,
            self.native.invalid_permille(),
            self.native.zero_cost_invalid,
            self.native.prevented,
            self.native.solved,
            render_option(self.native.bytes_per_solved()),
            self.shell.runs,
            self.shell.attempted,
            self.shell.invalid,
            self.shell.invalid_permille(),
            self.shell.solved,
            render_option(self.shell.bytes_per_solved()),
            render_option(self.reduction_percent()),
            self.clears(),
            self.shell_only_per_run_hundredths(),
        )
    }
}

fn render_option<T: core::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "undefined".to_owned(), |value| value.to_string())
}

/// What one misnameable position costs the baseline, under each argument mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionCost {
    /// The position.
    pub site: &'static Site,
    /// Interface bytes one misnamed argument at this position costs.
    pub misnamed_bytes: u64,
    /// Interface bytes one omitted argument at this position costs.
    pub omitted_bytes: u64,
}

/// The eight calls whose command lines carry every one of the 25 positions.
///
/// Fixed handles, because the *shape* of a command line is what carries a position and a
/// handle's value is not part of it. The handles are the same literals bn-2c0a's S5 attack
/// used, so the two measurements are comparable.
///
/// # Panics
///
/// Never: the three literals are well-formed handles.
#[must_use]
pub fn probe_calls() -> Vec<Call> {
    let snapshot = WorkspaceHandle::new("ws_0123456789abcdef").expect("a handle");
    let handle = TaskHandle::new("task_0123456789abcdef").expect("a handle");
    let continuation = ContinuationHandle::new("cont_0123456789abcdef").expect("a handle");
    vec![
        Call::CreateWorkspace { seal: false },
        Call::ForkModule {
            base: snapshot.clone(),
        },
        Call::SealWorkspace {
            snapshot: snapshot.clone(),
        },
        Call::StartVerification {
            snapshot,
            states: 64,
        },
        Call::PollTask {
            task: handle.clone(),
        },
        Call::FetchResult {
            task: handle.clone(),
        },
        Call::Resume {
            continuation,
            states: 64,
        },
        Call::Cancel { task: handle },
    ]
}

/// Price every one of the 25 misnameable positions, under both argument mistakes.
///
/// Exhaustive by construction: the eight calls above render every command line the surface
/// has, and every `--flag` in every one of them is priced. A position that could not be
/// misnamed would show a zero here.
#[must_use]
pub fn position_costs() -> Vec<PositionCost> {
    let mut out = Vec::new();
    for call in probe_calls() {
        let step = Step::faithful(call);
        let command = crate::shell::command_line(&SUBSET[0], &step);
        let operation = step.call.operation();
        for site in SITES.iter().filter(|site| site.operation == operation) {
            let (misnamed, misnamed_usage) = mistaken_command(
                &command,
                Mistake::MisnamedArgument {
                    ordinal: site.ordinal,
                },
            );
            let (omitted, omitted_usage) = mistaken_command(
                &command,
                Mistake::OmittedArgument {
                    ordinal: site.ordinal,
                },
            );
            out.push(PositionCost {
                site,
                misnamed_bytes: misnamed.len() as u64 + misnamed_usage.len() as u64,
                omitted_bytes: omitted.len() as u64 + omitted_usage.len() as u64,
            });
        }
    }
    out
}

/// The headline the exit bone needs: shell-only invalid attempts per run against the
/// break-even the landed instrument computed.
///
/// Three measured numbers rather than one, because "across families" has two honest readings
/// and pooling them would report neither. A family injects **one mistake per run**, so:
///
/// - [`Headline::prevented_hundredths`] pools the families whose class the typed surface has
///   no channel for. It answers "an agent that makes one argument mistake per run" — the
///   conservative reading, and the one [`Headline::clears`] is decided on.
/// - [`Headline::shared_hundredths`] pools the three classes both surfaces can express. Zero
///   is not filler: it is the measurement that the first number is not an artefact of which
///   classes were chosen.
/// - [`Headline::summed_hundredths`] adds the per-family rates rather than pooling their runs.
///   It answers "an agent that makes one mistake of *each* class per run". Pooling those
///   runs instead would divide two families' mistakes by five families' runs and report a
///   rate no agent has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Headline {
    /// The break-even, in hundredths of a shell-only invalid attempt per run, computed from
    /// the *landed* totals rather than restated:
    /// `crate::variants::invalid_reduction_breakeven_tenths` answered 6 tenths before these
    /// families existed.
    pub break_even_hundredths: i64,
    /// Measured, pooled over the families the typed surface has no channel for.
    pub prevented_hundredths: i64,
    /// Measured, pooled over the classes both surfaces can express. Zero is the anti-gaming
    /// evidence.
    pub shared_hundredths: i64,
    /// Measured, summed over every family: one mistake of each class per run.
    pub summed_hundredths: i64,
    /// How many of the prevented families' cells actually landed their mistake.
    ///
    /// Declared is one per cell; landed can be fewer, because a family stands aside from a
    /// step that already carries an IMPL-04 fault ([`Injection::apply`]). Printed beside the
    /// rate so a reader sees the declared rate and the achieved one rather than only the
    /// second.
    pub prevented_landed_cells: u32,
    /// How many cells the prevented families ran.
    pub prevented_cells: u32,
}

impl Headline {
    /// Whether the measured rate reaches the break-even.
    #[must_use]
    pub const fn clears(&self) -> bool {
        self.prevented_hundredths >= self.break_even_hundredths
    }
}

/// The fallible-policy family artifact.
///
/// Same rules as the two artifacts beside it (`crate::report`, `crate::falsification`): a
/// typed value, a total rendering function, deterministic order, no float, no clock, no path,
/// every section derived (INV-003, INV-005).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyReport {
    /// The headline measurement.
    pub headline: Headline,
    /// Every family's margin on every split, in report order.
    pub margins: Vec<FamilyMargin>,
    /// The 25 misnameable positions, priced.
    pub positions: Vec<PositionCost>,
    /// The falsification arms run against these families.
    pub attacks: Vec<crate::falsification::Attack>,
    /// What an adjudicator may rely on.
    pub surviving: Vec<String>,
}

impl FamilyReport {
    /// The canonical rendering.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("continuum-benchmark fallible-policy-families v1\n");
        out.push_str("# bn-2phq3. research/25 mistake classes on one shared policy. This\n");
        out.push_str("# artifact reports; it does not adjudicate G0-DX-10.\n");
        out.push('\n');

        out.push_str("[classes]\n");
        out.push_str("# class channel native shell asymmetric\n");
        for class in MistakeClass::ALL {
            out.push_str(&format!(
                "{} {} {} {} {}\n",
                class.token(),
                class.channel().token(),
                class.expressibility(Arm::Native).token(),
                class.expressibility(Arm::Shell).token(),
                class.asymmetric(),
            ));
        }
        out.push_str(&format!(
            "containment: typed channels are contained in text channels = {}\n\n",
            typed_channels_are_contained_in_text_channels(),
        ));

        out.push_str("[headline]\n");
        out.push_str(
            "# shell-only invalid attempts per run, in hundredths, against the break-even\n\
             # the LANDED instrument computed before these families existed. Each family\n\
             # injects exactly one mistake per run.\n",
        );
        out.push_str(&format!(
            "break_even_hundredths: {}\nprevented_families_hundredths: {}\n\
             shared_families_hundredths: {}\nsummed_over_families_hundredths: {}\n\
             prevented_mistakes_landed: {}/{} cells (declared one per cell)\nclears: {}\n\n",
            self.headline.break_even_hundredths,
            self.headline.prevented_hundredths,
            self.headline.shared_hundredths,
            self.headline.summed_hundredths,
            self.headline.prevented_landed_cells,
            self.headline.prevented_cells,
            self.headline.clears(),
        ));

        out.push_str("[margins]\n");
        out.push_str("# family class split <per-arm totals> reduction clears shell_only\n");
        for margin in &self.margins {
            out.push_str(&margin.render());
            out.push('\n');
        }
        out.push('\n');

        out.push_str("[positions]\n");
        out.push_str("# every place the text surface names an argument, and the\n");
        out.push_str("# compiler-resolved field the typed surface passes instead.\n");
        out.push_str("operation ordinal flag typed guaranteed misnamed_bytes omitted_bytes\n");
        for cost in &self.positions {
            out.push_str(&format!(
                "{} {} {} {} {} {} {}\n",
                cost.site.operation,
                cost.site.ordinal,
                cost.site.flag,
                cost.site.typed,
                cost.site.guaranteed,
                cost.misnamed_bytes,
                cost.omitted_bytes,
            ));
        }
        out.push_str(&format!(
            "positions: {}\nguaranteed: {}\n\n",
            self.positions.len(),
            self.positions
                .iter()
                .filter(|cost| cost.site.guaranteed)
                .count(),
        ));

        out.push_str("[attacks]\n");
        out.push_str("# id direction verdict :: family choice attacked :: measured evidence\n");
        for attack in &self.attacks {
            out.push_str(&attack.render());
            out.push('\n');
        }
        out.push('\n');

        out.push_str("[surviving]\n");
        for line in &self.surviving {
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');

        out.push_str("[limits]\n");
        out.push_str(
            "rate: the families declare one mistake per run and measure what it costs each \
             surface. That is a floor, not a rate: this instrument still cannot say how often \
             a real agent misnames an argument, and nothing here declares one.\n\
             agent: still scripted. A family varies what the agent gets wrong, not who the \
             agent is.\n\
             baseline: still a text projection of this daemon, not PR 13's CLI.\n\
             scope: this readies the invalid-action margin for the post-redesign re-run. The \
             bytes margin failed independently (bn-762i, X1 adjudicated), and nothing here \
             changes that or closes DX-10.\n\
             verdict: none. The DX-10 decision is bn-762i's.\n",
        );
        out
    }
}

/// Fold a family's runs into every split's margin, in report order.
#[must_use]
pub fn margins(family: MistakeFamily, native: &[ArmRun], shell: &[ArmRun]) -> Vec<FamilyMargin> {
    Split::all()
        .into_iter()
        .map(|split| FamilyMargin::measure(family, split, native, shell))
        .collect()
}

/// The shell-only invalid attempts per run, in hundredths, pooled over a set of margins.
///
/// Pooled over runs rather than averaged over families, so a family with fewer cells cannot
/// weigh as much as one with more. Pool only families that answer the *same* question: see
/// [`Headline`] for why pooling all five would report a rate no agent has.
#[must_use]
pub fn pooled_hundredths(margins: &[&FamilyMargin]) -> i64 {
    let runs: i64 = margins
        .iter()
        .map(|margin| i64::from(margin.shell.runs))
        .sum();
    if runs == 0 {
        return 0;
    }
    let shell_only: i64 = margins
        .iter()
        .map(|margin| i64::from(margin.shell.invalid) - i64::from(margin.native.invalid))
        .sum();
    (shell_only * 100) / runs
}

/// The shell-only invalid attempts per run, in hundredths, **summed** over a set of margins.
///
/// One mistake of each family's class per run. Additive across families rather than pooled,
/// which is the arithmetic the reading calls for and is stated as an assumption rather than
/// hidden: it holds exactly when the classes do not interact, and the per-family rows are
/// printed beside it so a reader can check that none of them moved the others.
#[must_use]
pub fn summed_hundredths(margins: &[&FamilyMargin]) -> i64 {
    margins
        .iter()
        .map(|margin| margin.shell_only_per_run_hundredths())
        .sum()
}

/// Recount one arm's attempts and invalid operations from its transcripts alone.
///
/// research/33's independent grader and full operation trace, applied to this module's own
/// numbers: the transcript is written by `crate::run` from the [`Observation`] an arm
/// returned, and every count in a [`FamilyMargin`] can be re-derived from it without reading
/// a counter. `tests/pr10_fallible_families.rs` re-derives them and compares, so a counter
/// that drifted from the trace is caught rather than trusted.
///
/// Returns `(attempted, invalid, prevented)`.
///
/// [`Observation`]: crate::surface::Observation
#[must_use]
pub fn regrade(runs: &[ArmRun]) -> (u32, u32, u32) {
    let mut attempted = 0;
    let mut invalid = 0;
    let mut prevented = 0;
    for run in runs {
        for line in &run.transcript {
            if line.starts_with("stuck ") {
                continue;
            }
            if line.contains(" expressed=false ") {
                prevented += 1;
                continue;
            }
            attempted += 1;
            if line.contains(" expressed=true ") || line.contains(" admitted=false ") {
                invalid += 1;
            }
        }
    }
    (attempted, invalid, prevented)
}

/// Every family's runs, per arm, keyed by family identifier.
///
/// A convenience for the artifact builder: the native arm's sweep lives in `tests/` (plan §20
/// keeps `continuum-mcp` out of this library), so the two halves are assembled by the caller
/// and this type is what they are assembled into.
pub type FamilyRuns = BTreeMap<&'static str, (Vec<ArmRun>, Vec<ArmRun>)>;
