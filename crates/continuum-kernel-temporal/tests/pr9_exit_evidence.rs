//! Dedicated exit evidence for `PR-9-EXIT` (`notes/plan/notes/START_HERE_IMPLEMENTATION.md`,
//! PR 9's Exit line; delivered across bn-2i8 and bn-2he).
//!
//! > **Exit:** every single-field certificate mutation in the test suite is rejected,
//! > checking wire-form input only.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 9
//!
//! See `continuum-kernel-core/tests/pr9_exit_evidence.rs` for the shared rationale this
//! file repeats per crate. PR 9's sentence already has a mechanical, cross-crate index
//! — `tools/kernel-covenant/mutation-matrix.toml` (bn-2he) and `tools/
//! check_kernel_covenant.py`'s fail-closed KCOV-08 — and this crate's own
//! `src/check.rs` and `tests/wire_form_boundary.rs` already carry most of the
//! sentence's weight as exhaustive field sweeps. This file adds the residual:
//! single-field mutations this crate owns in the matrix but has so far only exercised
//! internally (via `Plan`), now exercised from wire-form bytes alone, and a mechanical
//! check that "checking wire-form input only" still holds against this crate's own
//! sources.
//!
//! # Evidence map
//!
//! - **"every single-field certificate mutation … is rejected"** — carried
//!   exhaustively by `check::tests::removing_any_transition_target_from_the_table_breaks_the_certificate`,
//!   `check::tests::the_carried_ranking_is_minimal_so_no_rank_can_shrink`,
//!   `check::tests::a_multi_state_component_is_found_and_judged_as_one`,
//!   `check::tests::no_two_byte_strings_decode_to_the_same_certificate`, and
//!   `check::tests::garbage_bytes_never_panic` (`src/check.rs`), and from outside the
//!   crate by `weak_fairness_is_what_separates_a_stall_from_a_counterexample` (`tests/
//!   wire_form_boundary.rs`) — already the `body.reference-retarget` class
//!   (`Rejection::FairCycleExists`), from wire-form bytes alone. `envelope.digest-
//!   relabelling` is `uncovered` for this crate in the matrix, a tracked gap bn-1q4r7
//!   owns and out of scope here. Three classes are added here that
//!   `wire_form_boundary.rs` does not yet touch:
//!   [`an_envelope_wire_epoch_this_build_does_not_implement_is_unsupported_from_bytes_alone`]
//!   (`framing.wire-epoch-substitution`, corroborated internally by
//!   `check::tests::unimplemented_contracts_are_unsupported_not_rejected`),
//!   [`a_domain_pack_count_beyond_the_declared_ceiling_is_rejected_from_bytes_alone`]
//!   (`encoding.count-out-of-range`, corroborated internally by
//!   `check::tests::counts_outside_the_declared_range_are_rejected`), and
//!   [`a_fair_action_beyond_the_declared_action_count_is_rejected_from_bytes_alone`],
//!   which repeats `check::tests::a_fair_action_outside_the_action_table_is_rejected`'s
//!   `body.dangling-reference` claim (`Rejection::FairActionOutOfRange`) from wire-form
//!   bytes alone — a different field, and a different certificate family
//!   (fair-SCC-exclusion rather than ranking), from the reference-retarget mutation
//!   `wire_form_boundary.rs` already shows.
//! - **"checking wire-form input only"** — `src/lib.rs`'s own module documentation:
//!   "`check_certificate` takes `&[u8]`. It is the only public verdict-returning `fn`, and no
//!   constructor exists for `wire::Certificate` outside `wire::decode`." Checked
//!   mechanically here, against `include_str!`-loaded copies of this crate's own
//!   sources, the same idiom `continuum_intent::tests::pr4_exit_evidence`'s
//!   `no_type_in_this_crate_has_a_mut_self_method` uses.
//!
//! # House rules
//!
//! `src/` is not touched; no existing test is touched; the matrix and covenant checker
//! are cited, not re-implemented; the encoder is duplicated locally from `tests/
//! wire_form_boundary.rs`; `envelope.digest-relabelling` is not touched anywhere in
//! this file (bn-1q4r7's fence).

use continuum_kernel_temporal::wire::{MAGIC, WIRE_EPOCH, decode};
use continuum_kernel_temporal::{
    CertificateKind, FairnessClass, Feature, Rejection, Verdict, check_certificate,
};

// --- the encoder, duplicated from `tests/wire_form_boundary.rs` -----------------------

/// A minimal encoder, written against the documented grammar.
#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn u16(&mut self, value: u16) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn u32(&mut self, value: u32) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn i64(&mut self, value: i64) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn token(&mut self, value: &str) -> &mut Self {
        self.u16(u16::try_from(value.len()).expect("fixture tokens are short"));
        self.0.extend_from_slice(value.as_bytes());
        self
    }

    fn header(&mut self, kind: u16, domain_pack_count: u16) -> &mut Self {
        self.0.extend_from_slice(&MAGIC);
        self.u16(WIRE_EPOCH);
        self.u16(kind);
        self.token("blake3:countdown-model");
        self.token("continuum-semantics-1");
        self.token("blake3:eventually-zero");
        self.token("blake3:countdown-scope");
        self.token("blake3:empty-assumptions");
        self.token("continuum-engine-liveness/0.0.0");
        self.u16(WIRE_EPOCH);
        self.u16(domain_pack_count)
    }
}

/// The same graph as `wire_form_boundary.rs`'s `countdown_with_stall`, with a `stall`
/// self-loop at `n = 2` and `tick` declared weakly fair, parameterized over the three
/// fields this file mutates.
///
/// - `domain_pack_count` — the envelope's declared domain-pack count. `0` (green)
///   declares none.
/// - `fair_action` — the one weakly-fair action's index. `1` (green) names `tick`;
///   anything `>=` the two-action count is out of range.
fn countdown_with_fair_tick_shaped(domain_pack_count: u16, fair_action: u16) -> Vec<u8> {
    let mut out = Bytes::default();
    out.header(2, domain_pack_count);

    out.u16(1); // one variable
    out.token("n");
    out.i64(0);
    out.i64(2);

    out.u32(3); // three states
    out.i64(0);
    out.i64(1);
    out.i64(2);

    out.u16(1); // property class: eventually-state-set

    out.u32(1); // goal: n = 0
    out.i64(0);

    out.u32(1); // initial: n = 2
    out.i64(2);

    out.u16(2); // two actions, strictly ascending
    out.token("stall");
    out.token("tick");

    out.u16(1); // weak fairness
    out.u16(1); // one fair action
    out.u16(fair_action);

    out.u32(0); // n = 0: goal, no transitions
    out.u32(1); // n = 1
    out.u16(1); // tick
    out.i64(0);
    out.u32(2); // n = 2: stall to itself, or tick down
    out.u16(0); // stall
    out.i64(2);
    out.u16(1); // tick
    out.i64(1);

    out.0
}

/// The green certificate: `wire_form_boundary.rs`'s `countdown_with_stall(true)`, byte
/// for byte.
fn countdown_with_fair_tick() -> Vec<u8> {
    countdown_with_fair_tick_shaped(0, 1)
}

// --- clause 2: "checking wire-form input only" ----------------------------------------

const WIRE_SRC: &str = include_str!("../src/wire.rs");

/// Whether `source` contains `-> {exact_type}` as a bare return type, not merely as a
/// prefix of a longer identifier.
fn contains_bare_return_type(source: &str, exact_type: &str) -> bool {
    let marker = format!("-> {exact_type}");
    let mut search_from = 0usize;
    while let Some(pos) = source.get(search_from..).and_then(|s| s.find(&marker)) {
        let hit = search_from + pos;
        let after = source.get(hit + marker.len()..).unwrap_or("");
        let is_bare = match after.chars().next() {
            Some(next) => !next.is_alphanumeric() && next != '_',
            None => true,
        };
        if is_bare {
            return true;
        }
        search_from = hit + marker.len();
    }
    false
}

#[test]
fn checking_has_exactly_one_public_entry_point_and_it_takes_wire_form_bytes() {
    let mut public_verdict_producers = Vec::new();
    for (name, source) in walked_sources(&[
        "check.rs",
        "fixture.rs",
        "lib.rs",
        "receipt.rs",
        "verdict.rs",
        "wire.rs",
    ]) {
        public_verdict_producers.extend(verdict_producers(&name, &source, "Verdict"));
    }
    assert_eq!(
        public_verdict_producers,
        vec!["check.rs: pub fn check_certificate(bytes: &[u8]) -> Verdict".to_owned()],
        "the crate must expose exactly one public route to a Verdict, and it must take \
         wire-form bytes; found {public_verdict_producers:?}"
    );
}

#[test]
fn certificate_has_no_public_constructor_no_public_field_and_no_manual_trait_impl() {
    let struct_start = WIRE_SRC
        .find("pub struct Certificate {")
        .expect("Certificate is declared in wire.rs");
    let struct_tail = &WIRE_SRC[struct_start..];
    let struct_end = struct_tail
        .find("\n}\n")
        .expect("the struct body has a closing brace");
    let struct_block = &struct_tail[..struct_end];
    for line in struct_block.lines().skip(1) {
        let trimmed = line.trim_start();
        assert!(
            !trimmed.starts_with("pub "),
            "Certificate grew a public field ({trimmed:?}); an external caller could then \
             build one without going through decode"
        );
    }

    let impl_start = WIRE_SRC
        .find("impl Certificate {")
        .expect("Certificate has an inherent impl");
    let impl_tail = &WIRE_SRC[impl_start..];
    let impl_end = impl_tail
        .find("\n}\n")
        .expect("the impl body has a closing brace");
    let impl_block = &impl_tail[..impl_end];
    assert!(
        !impl_block.contains("fn new("),
        "Certificate's inherent impl grew a `new` constructor outside `decode`"
    );
    assert!(
        !impl_block.contains("-> Self"),
        "Certificate's inherent impl grew a method that can mint one from `&self`"
    );
    assert!(
        !contains_bare_return_type(impl_block, "Certificate"),
        "Certificate's inherent impl grew a method returning a fresh Certificate rather \
         than borrowing the one `decode` already built"
    );

    assert!(
        !WIRE_SRC.contains("for Certificate"),
        "an `impl … for Certificate` appeared (From/Default/etc. would each read `for \
         Certificate`); decode is meant to be the only way in"
    );
}

// --- clause 1: three more single-field mutations, exercised from wire-form bytes alone -

#[test]
fn an_envelope_wire_epoch_this_build_does_not_implement_is_unsupported_from_bytes_alone() {
    let mut bytes = countdown_with_fair_tick();
    let unimplemented_epoch = WIRE_EPOCH.saturating_add(1);
    let patch = unimplemented_epoch.to_be_bytes();
    bytes[8] = patch[0];
    bytes[9] = patch[1];

    assert_eq!(
        check_certificate(&bytes),
        Verdict::Unsupported(Feature::WireEpoch {
            found: unimplemented_epoch
        }),
        "a wire epoch this build does not implement must be Unsupported, never Rejected"
    );

    assert!(check_certificate(&countdown_with_fair_tick()).is_verified());
}

#[test]
fn a_domain_pack_count_beyond_the_declared_ceiling_is_rejected_from_bytes_alone() {
    const MAX_DOMAIN_PACKS: u16 = 64;
    let bytes = countdown_with_fair_tick_shaped(MAX_DOMAIN_PACKS.saturating_add(1), 1);

    let Verdict::Rejected(rejection) = check_certificate(&bytes) else {
        panic!("a domain-pack count above the declared ceiling must be rejected");
    };
    assert_eq!(
        rejection,
        Rejection::CountOutOfRange {
            field: continuum_kernel_temporal::Field::DomainPackCount,
            found: u64::from(MAX_DOMAIN_PACKS.saturating_add(1)),
            min: 0,
            max: u64::from(MAX_DOMAIN_PACKS),
        }
    );
}

#[test]
fn a_fair_action_beyond_the_declared_action_count_is_rejected_from_bytes_alone() {
    // Only two actions are declared (`stall` at 0, `tick` at 1); fair action index `5`
    // names an action the certificate never introduced.
    let bytes = countdown_with_fair_tick_shaped(0, 5);

    assert_eq!(
        check_certificate(&bytes),
        Verdict::Rejected(Rejection::FairActionOutOfRange {
            position: 0,
            action: 5,
            actions: 2,
        })
    );
}

// --- the shaped fixture still agrees with the original, unmutated ---------------------

#[test]
fn the_shaped_green_fixture_still_matches_the_original_countdown_with_stall() {
    let bytes = countdown_with_fair_tick();
    let Verdict::Verified(claim) = check_certificate(&bytes) else {
        panic!("the unmutated shaped fixture must still check green");
    };
    assert_eq!(claim.kind(), CertificateKind::FairSccExclusion);
    assert_eq!(claim.fairness(), FairnessClass::Weak);
    let redecoded = decode(&bytes).expect("the green fixture decodes");
    assert_eq!(redecoded.kind(), CertificateKind::FairSccExclusion);
}

// --- the entry-point scan (Codex cr-10mg1y round 4) ------------------------------------

/// Every file under this crate's `src/`, walked from disk, as (relative path, text).
///
/// A fixed list of `include_str!`s cannot see a module it does not name, and a
/// `#[path]` module or a non-`.rs` file would sit outside it. So the list is walked,
/// and any file that is not in `expected` fails this check, whatever its extension.
fn walked_sources(expected: &[&str]) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![root.clone()];
    let mut out = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("src/ is readable") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(&root)
                .expect("the walk stays under src/")
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("src/{rel} is not readable text: {error}"));
            out.push((rel, text));
        }
    }
    out.sort();
    let found: Vec<&str> = out.iter().map(|(rel, _)| rel.as_str()).collect();
    assert_eq!(
        found, expected,
        "src/ holds a file this check does not expect; a module it cannot see is a way \
         around it"
    );
    out
}

/// Every `pub fn` / `pub const fn` whose return type names `ty` as a path segment,
/// each signature normalised to one line. Raw identifiers (`r#try`), qualified return
/// paths (`crate::verdict::Verdict`, `super::Verdict`) and signatures split over lines
/// are all seen.
fn verdict_producers(name: &str, source: &str, ty: &str) -> Vec<String> {
    let code = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join(" ");
    let flat = code.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = Vec::new();
    let mut rest = flat.as_str();
    while let Some(start) = ["pub fn ", "pub const fn "]
        .iter()
        .filter_map(|marker| rest.find(marker))
        .min()
    {
        let tail = &rest[start..];
        let end = tail.find(['{', ';']).unwrap_or(tail.len());
        let signature = tail[..end].trim();
        if let Some((_, returned)) = signature.split_once("->")
            && returned
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .any(|segment| segment == ty)
        {
            out.push(format!("{name}: {signature}"));
        }
        rest = &tail[end.max(1)..];
    }
    out
}

#[test]
fn the_entry_point_scan_sees_raw_identifiers_qualified_returns_and_split_signatures() {
    let planted = "pub fn r#try(decoded: &crate::wire::Certificate) -> crate::verdict::TY {\n}\n\
                   pub fn split(\n    decoded: &Certificate,\n) -> super::TY {\n}\n\
                   pub const fn quiet(x: u8) -> u8 {\n    x\n}\n\
                   pub fn named(x: u8) -> TYAndMore {\n}\n"
        .replace("TY", "Verdict");
    assert_eq!(
        verdict_producers("planted.rs", &planted, "Verdict"),
        [
            "planted.rs: pub fn r#try(decoded: &crate::wire::Certificate) -> crate::verdict::Verdict",
            "planted.rs: pub fn split( decoded: &Certificate, ) -> super::Verdict",
        ],
        "a raw identifier, a qualified return and a split signature are producers; a \
         longer identifier that merely starts with the type's name is not"
    );
}

// --- public values that could carry a verdict or a callable (Codex cr-3i3rst round 6) ---

/// The verdict types of the checking base. A public value whose type names one, or any
/// callable (`Fn`, `FnMut`, `FnOnce`, `fn(`, `dyn`, `impl `), is refused: the scan
/// refuses the whole syntactic class rather than proving an instance wire-only.
const VERDICT_TYPES: [&str; 4] = ["Verdict", "Outcome", "KernelVerdict", "CheckedClaim"];

fn flat_code(source: &str) -> String {
    let code = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join(" ");
    code.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether a type's text names a callable.
fn carries_callable(ty: &str) -> bool {
    let squeezed: String = ty.chars().filter(|c| !c.is_whitespace()).collect();
    squeezed.contains("fn(")
        || ty.contains("impl ")
        || ty
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .any(|word| matches!(word, "Fn" | "FnMut" | "FnOnce" | "dyn"))
}

/// Whether a type's text names a callable or a verdict type.
fn carries_callable_or_verdict(ty: &str) -> bool {
    carries_callable(ty)
        || ty
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .any(|word| VERDICT_TYPES.contains(&word))
}

/// The byte offset of the first `stop` at depth zero, skipping `->`.
fn depth_zero(text: &str, stops: &[char]) -> usize {
    let bytes: Vec<char> = text.chars().collect();
    let mut depth = 0_i32;
    let mut offset = 0;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == '-' && bytes.get(i + 1) == Some(&'>') {
            offset += 2;
            i += 2;
            continue;
        }
        if depth == 0 && stops.contains(&c) {
            return offset;
        }
        match c {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth -= 1,
            _ => {}
        }
        offset += c.len_utf8();
        i += 1;
    }
    text.len()
}

/// The text inside the delimiter that opens at the start of `text`.
fn delimited(text: &str) -> &str {
    let inner = &text[1..];
    &inner[..depth_zero(inner, &[')', '}'])]
}

/// Split at depth-zero commas.
fn fields(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.trim().is_empty() {
        let end = depth_zero(rest, &[',']);
        out.push(rest[..end].trim());
        rest = rest.get(end + 1..).unwrap_or("");
    }
    out
}

/// Every public value, field or trait method that could carry a verdict or a callable.
fn public_values(name: &str, source: &str) -> Vec<String> {
    let flat = flat_code(source);
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = flat[from..].find("pub ") {
        let at = from + found;
        from = at + 4;
        let boundary = flat[..at]
            .chars()
            .last()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        if !boundary {
            continue;
        }
        let rest = &flat[at + 4..];
        if (rest.starts_with("static ") || rest.starts_with("const "))
            && !rest.starts_with("const fn ")
        {
            let item = &rest[..depth_zero(rest, &[';'])];
            let ty = item
                .split_once(':')
                .map_or("", |(_, t)| t.split_once('=').map_or(t, |(t, _)| t));
            if carries_callable_or_verdict(ty) {
                out.push(format!("{name}: pub {item}"));
            }
        } else if rest.starts_with("type ") {
            let item = &rest[..depth_zero(rest, &[';'])];
            if carries_callable_or_verdict(item.split_once('=').map_or("", |(_, t)| t)) {
                out.push(format!("{name}: pub {item}"));
            }
        } else if let Some(body) = rest.strip_prefix("trait ") {
            let open = body.find('{').unwrap_or(body.len());
            for method in delimited(&body[open..]).split("fn ").skip(1) {
                let signature = &method[..depth_zero(method, &[';', '{'])];
                if signature
                    .split_once("->")
                    .is_some_and(|(_, r)| carries_callable_or_verdict(r))
                {
                    out.push(format!("{name}: pub trait method fn {}", signature.trim()));
                }
            }
        } else if let Some(body) = rest.strip_prefix("struct ") {
            let open = body.find(['{', '(', ';']).unwrap_or(body.len());
            if matches!(body[open..].chars().next(), Some('{' | '(')) {
                for field in fields(delimited(&body[open..])) {
                    if let Some(public) = field.strip_prefix("pub ") {
                        let ty = public.split_once(':').map_or(public, |(_, t)| t);
                        if carries_callable_or_verdict(ty) {
                            out.push(format!("{name}: pub field {field}"));
                        }
                    }
                }
            }
        } else if let Some(body) = rest.strip_prefix("enum ") {
            let enum_name: String = body
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            // A verdict enum carries its own claim, which is what a verdict is; it may
            // still hold no callable.
            let own_verdict = VERDICT_TYPES.contains(&enum_name.as_str());
            let open = body.find('{').unwrap_or(body.len());
            if open < body.len() {
                for variant in fields(delimited(&body[open..])) {
                    if let Some(start) = variant.find(['{', '(']) {
                        for field in fields(delimited(&variant[start..])) {
                            let ty = field.split_once(':').map_or(field, |(_, t)| t);
                            let flagged = if own_verdict {
                                carries_callable(ty)
                            } else {
                                carries_callable_or_verdict(ty)
                            };
                            if flagged {
                                out.push(format!("{name}: pub enum {enum_name} field {field}"));
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

#[test]
fn no_public_value_can_carry_a_verdict_or_a_callable() {
    let mut found = Vec::new();
    for (name, source) in walked_sources(&[
        "check.rs",
        "fixture.rs",
        "lib.rs",
        "receipt.rs",
        "verdict.rs",
        "wire.rs",
    ]) {
        found.extend(public_values(&name, &source));
    }
    assert!(
        found.is_empty(),
        "a public static, const, field, alias or trait method carries a callable or a \
         verdict type; the scan refuses the whole class: {found:?}"
    );
}

#[test]
fn the_public_value_scan_refuses_callable_and_verdict_carrying_values() {
    let planted = "pub static DECIDE_DECODED: &(dyn Fn(&crate::wire::Certificate) -> Verdict + Sync) = &|c| { check(c) };\n\
                   pub const DECIDE: fn(&crate::wire::Certificate) -> Verdict = decide_impl;\n\
                   pub struct Decider {\n    pub decide: fn(&crate::wire::Certificate) -> Verdict,\n    pub count: u32,\n}\n\
                   pub struct Holder;\nimpl Holder {\n    pub const DECIDE: fn(&crate::wire::Certificate) -> Verdict = decide_impl;\n}\n\
                   pub trait Decides {\n    fn decide(&self, c: &crate::wire::Certificate) -> Verdict;\n}\n\
                   pub const MAGIC: [u8; 8] = *b\"CONTCERT\";\n";
    let found = public_values("planted.rs", planted);
    assert_eq!(found.len(), 5, "{found:?}");
    assert!(
        found
            .iter()
            .all(|f| !f.contains("MAGIC") && !f.contains("count")),
        "{found:?}"
    );
}
