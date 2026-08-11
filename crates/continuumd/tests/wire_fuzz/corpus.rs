//! The persistent corpus: a committed, text-readable case file per input.
//!
//! # Why a text format and not raw bytes
//!
//! A corpus of anonymous `.bin` blobs is a corpus nobody reviews. Every case here
//! carries, in the file, the four things a reader needs to judge it without running it:
//! which target it is for, which adversarial class it belongs to, **where it must land**,
//! and where it came from. The bytes are hex, so a diff of a corpus change is a diff a
//! human can read, and `git` stores it as text.
//!
//! # Why the landing is in the file
//!
//! Because "a seed that the codec rejects at the frame layer before the case's actual
//! property is exercised is not coverage". The `expect` line is the assertion that the
//! case still reaches the rule it was written for; `wire_fuzz.rs` checks every committed
//! case against it, so a decoder change that starts refusing a duplicate-id seed one
//! layer earlier fails the gate instead of quietly turning the seed into decoration.
//!
//! # Format
//!
//! ```text
//! # continuum wire-fuzz case, format 1
//! target = canonical.json
//! class = cyclic
//! expect = json::too-deep
//! origin = seed
//! note = one-sided nesting to MAX_DEPTH + 1
//! bytes =
//!   5b5b5b5b
//! ```
//!
//! Keys are fixed, ordered, and all required. Hex is lowercase, at most 64 characters per
//! continuation line, and a case with no bytes writes an empty `bytes =` with no
//! continuation lines.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Which adversarial class a case belongs to.
///
/// The first four are the classes the bone requires seeded. `WellFormed` is the control —
/// a corpus with no accepted inputs cannot tell "rejects everything" from "rejects the
/// right things". `Regression` is what a minimized finding becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    /// Unbounded self-nesting: the serialized form of a cycle in a format with no
    /// back-references.
    Cyclic,
    /// A declared size or count beyond the production bound. Always declared, never
    /// materialized.
    Oversized,
    /// The same identity twice where the format requires uniqueness.
    DuplicateId,
    /// A second spelling of a value that already has one.
    Noncanonical,
    /// A valid input. The control.
    WellFormed,
    /// A minimized finding, kept as a named regression.
    Regression,
}

impl Class {
    /// The stable machine token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cyclic => "cyclic",
            Self::Oversized => "oversized",
            Self::DuplicateId => "duplicate-id",
            Self::Noncanonical => "noncanonical",
            Self::WellFormed => "well-formed",
            Self::Regression => "regression",
        }
    }

    /// Parse a token.
    #[must_use]
    pub fn parse(token: &str) -> Option<Self> {
        [
            Self::Cyclic,
            Self::Oversized,
            Self::DuplicateId,
            Self::Noncanonical,
            Self::WellFormed,
            Self::Regression,
        ]
        .into_iter()
        .find(|class| class.as_str() == token)
    }

    /// The four classes the bone requires seeded.
    pub const REQUIRED: [Self; 4] = [
        Self::Cyclic,
        Self::Oversized,
        Self::DuplicateId,
        Self::Noncanonical,
    ];
}

/// Where a case came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    /// Written by hand against a documented production bound.
    Seed,
    /// Produced by the campaign and shrunk by the deterministic minimizer.
    Minimized,
}

impl Origin {
    /// The stable machine token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Seed => "seed",
            Self::Minimized => "minimized",
        }
    }

    /// Parse a token.
    #[must_use]
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "seed" => Some(Self::Seed),
            "minimized" => Some(Self::Minimized),
            _ => None,
        }
    }
}

/// One committed corpus case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// The file's stem, which is also the regression's name.
    pub name: String,
    /// The target this case is for.
    pub target: String,
    /// Which adversarial class it belongs to.
    pub class: Class,
    /// The landing token this case MUST reach.
    pub expect: String,
    /// Where the case came from.
    pub origin: Origin,
    /// Why this case exists, in one line.
    pub note: String,
    /// The input, verbatim.
    pub bytes: Vec<u8>,
}

/// Why a case file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseError {
    /// A required key is absent.
    MissingKey(&'static str),
    /// A key appeared that the format does not declare.
    UnknownKey(String),
    /// A value is not a member of its closed vocabulary.
    BadValue {
        /// The key.
        key: &'static str,
        /// The offending token.
        found: String,
    },
    /// The hex payload is not lowercase hex in whole octets.
    BadHex(String),
    /// A line is neither a comment, a `key = value`, nor a hex continuation.
    BadLine(String),
}

impl core::fmt::Display for CaseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingKey(key) => write!(f, "the case declares no `{key}`"),
            Self::UnknownKey(key) => write!(f, "`{key}` is not a case key"),
            Self::BadValue { key, found } => write!(f, "`{key} = {found}` is not admissible"),
            Self::BadHex(line) => write!(f, "`{line}` is not lowercase hex octets"),
            Self::BadLine(line) => write!(f, "`{line}` is not a case line"),
        }
    }
}

/// The header every case file carries.
pub const HEADER: &str = "# continuum wire-fuzz case, format 1";

/// Hex characters per continuation line.
const HEX_WIDTH: usize = 64;

impl Case {
    /// Render this case to its file form.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(HEADER);
        out.push('\n');
        let _ = writeln!(out, "target = {}", self.target);
        let _ = writeln!(out, "class = {}", self.class.as_str());
        let _ = writeln!(out, "expect = {}", self.expect);
        let _ = writeln!(out, "origin = {}", self.origin.as_str());
        let _ = writeln!(out, "note = {}", self.note);
        out.push_str("bytes =\n");
        let mut hex = String::with_capacity(self.bytes.len().saturating_mul(2));
        for byte in &self.bytes {
            let _ = write!(hex, "{byte:02x}");
        }
        let characters: Vec<char> = hex.chars().collect();
        for chunk in characters.chunks(HEX_WIDTH) {
            out.push_str("  ");
            out.extend(chunk.iter());
            out.push('\n');
        }
        out
    }

    /// Read a case from its file form.
    ///
    /// # Errors
    ///
    /// [`CaseError`] naming the first line the format does not admit. Strict on purpose:
    /// a corpus loader that repaired its own input would be the last place to notice that
    /// a case had rotted.
    pub fn parse(name: &str, text: &str) -> Result<Self, CaseError> {
        let mut target: Option<String> = None;
        let mut class: Option<Class> = None;
        let mut expect: Option<String> = None;
        let mut origin: Option<Origin> = None;
        let mut note: Option<String> = None;
        let mut hex = String::new();
        let mut in_bytes = false;

        for line in text.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("  ") {
                if !in_bytes {
                    return Err(CaseError::BadLine(line.to_owned()));
                }
                hex.push_str(rest.trim_end());
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(CaseError::BadLine(line.to_owned()));
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "target" => target = Some(value.to_owned()),
                "class" => {
                    class = Some(Class::parse(value).ok_or_else(|| CaseError::BadValue {
                        key: "class",
                        found: value.to_owned(),
                    })?);
                }
                "expect" => expect = Some(value.to_owned()),
                "origin" => {
                    origin = Some(Origin::parse(value).ok_or_else(|| CaseError::BadValue {
                        key: "origin",
                        found: value.to_owned(),
                    })?);
                }
                "note" => note = Some(value.to_owned()),
                "bytes" => {
                    in_bytes = true;
                    if !value.is_empty() {
                        return Err(CaseError::BadLine(line.to_owned()));
                    }
                }
                other => return Err(CaseError::UnknownKey(other.to_owned())),
            }
        }

        if !in_bytes {
            return Err(CaseError::MissingKey("bytes"));
        }
        if hex.len() % 2 != 0
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(CaseError::BadHex(hex));
        }
        let mut bytes = Vec::with_capacity(hex.len() / 2);
        let characters: Vec<char> = hex.chars().collect();
        for pair in characters.chunks(2) {
            let text: String = pair.iter().collect();
            let byte = u8::from_str_radix(&text, 16).map_err(|_| CaseError::BadHex(text))?;
            bytes.push(byte);
        }

        Ok(Self {
            name: name.to_owned(),
            target: target.ok_or(CaseError::MissingKey("target"))?,
            class: class.ok_or(CaseError::MissingKey("class"))?,
            expect: expect.ok_or(CaseError::MissingKey("expect"))?,
            origin: origin.ok_or(CaseError::MissingKey("origin"))?,
            note: note.ok_or(CaseError::MissingKey("note"))?,
            bytes,
        })
    }
}

/// The committed corpus root.
///
/// Resolved from `CARGO_MANIFEST_DIR` rather than the working directory: a gate whose
/// input depends on where it was invoked from is not reproducible (INV-005).
#[must_use]
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("wire-fuzz-corpus")
}

/// Every case under `root`, sorted by (target, name).
///
/// The sort is load-bearing: `read_dir` order is a filesystem property, and a campaign
/// seeded from an unsorted corpus would produce different inputs on different machines.
///
/// # Panics
///
/// When the corpus directory cannot be read or a case file does not parse. Both are
/// failures of the corpus itself, and a loader that skipped them would report the
/// coverage of a corpus it did not load.
#[must_use]
pub fn load() -> Vec<Case> {
    let mut cases = Vec::new();
    let root = root();
    let mut targets: Vec<PathBuf> = fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("the corpus root {} is unreadable: {error}", root.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("a corpus entry is unreadable: {error}"))
                .path()
        })
        .filter(|path| path.is_dir())
        .collect();
    targets.sort();
    for directory in targets {
        let mut files: Vec<PathBuf> = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{} is unreadable: {error}", directory.display()))
            .map(|entry| {
                entry
                    .unwrap_or_else(|error| panic!("a corpus entry is unreadable: {error}"))
                    .path()
            })
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "case")
            })
            .collect();
        files.sort();
        for file in files {
            let name = file
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_else(|| panic!("{} has no usable stem", file.display()))
                .to_owned();
            let text = fs::read_to_string(&file)
                .unwrap_or_else(|error| panic!("{} is unreadable: {error}", file.display()));
            let case = Case::parse(&name, &text)
                .unwrap_or_else(|error| panic!("{} is not a case: {error}", file.display()));
            cases.push(case);
        }
    }
    cases
}

/// Write `case` under `directory`, creating the target subdirectory.
///
/// Used by the crash round trip: a finding is minimized, written here, and re-read. The
/// path returned is what a failing campaign prints, so the file a reviewer is told to
/// commit is the file the harness actually wrote.
///
/// # Panics
///
/// When the file cannot be written. There is nothing useful to do with a finding that
/// cannot be recorded.
pub fn write(directory: &Path, case: &Case) -> PathBuf {
    let target_directory = directory.join(&case.target);
    fs::create_dir_all(&target_directory).unwrap_or_else(|error| {
        panic!("{} is not writable: {error}", target_directory.display());
    });
    let path = target_directory.join(format!("{}.case", case.name));
    fs::write(&path, case.render())
        .unwrap_or_else(|error| panic!("{} is not writable: {error}", path.display()));
    path
}
