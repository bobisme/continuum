//! PR-22A / IMPL-03 (bn-3gf6): the measured invalidation-precision baseline.
//!
//! It measures how precise the spike's invalidation is, and compares the result
//! with the module-granularity baseline. The engine is not changed. This test only
//! observes what the engine does, and it judges each observation against the
//! independent clean recomputation (`audit::clean`, INV-010).
//!
//! # Definitions
//!
//! The unit is one *compared* query label (parse, elaborate, explore, domain_safety,
//! check_invariant) of one audited revision `before → after`. `clean(x)` is the
//! clean output of that label for source `x`, if the clean run has the label.
//!
//! - **changed(L)**: `clean(before)` is absent or differs from `clean(after)`. If a
//!   query is recomputed and its label changed, the recompute was necessary.
//! - **TP** (necessary recompute): the engine recomputed L and changed(L).
//! - **FP** (over-invalidation): the engine recomputed L, but `clean(before)` exists
//!   and equals `clean(after)`. The output did not change. This costs latency only.
//! - **FN** (under-invalidation): the engine reused L, and the reused output differs
//!   from `clean(after)`. In the promotion lane, or through an `Exact`, `Validated` or
//!   `Conservative` licence, this is the one unrecoverable error (RFC 0030). Through
//!   an `Experimental` licence in the interactive lane it is an `experimental miss`:
//!   it is measured, and it can never support promotion.
//! - **TN** (correct reuse): the engine reused L, and the reused output equals
//!   `clean(after)`. `tn_label_changed` counts the subset where changed(L) is also
//!   true. These are content-addressed reuses across a label change (for example, a
//!   renamed invariant whose check has the same key).
//! - **only in engine**: the engine produced L and the clean run of the edited
//!   source has no L (a stale parse let the engine go on past a parse error). It
//!   is a phantom artifact, counted apart and outside TP, FP, FN, TN and recall.
//! - **only in clean**: the clean run of the edited source has L and the engine did
//!   not produce it (a stale reuse upstream stopped the pipeline). It is a recall
//!   miss: counted in FN, and attributed to the licence the audit blames for the
//!   first divergence.
//! - **recompute_wrong**: a recomputed L whose output differs from `clean(after)`.
//!   Asserted zero.
//! - **precision** = TP / (TP + FP): the share of recomputes that were necessary.
//! - **recall** = TP / (TP + FN): the share of necessary invalidations that were
//!   made. A correct content-addressed reuse is neither a TP nor an FN.
//!
//! The internal queries (build_model and the projections) have no clean counterpart,
//! so they are not judged. They are counted separately under `internal`.
//!
//! **Module-granularity baseline.** Every corpus model is one file, and every edit
//! changes its bytes. So module-granularity invalidation recomputes every compared
//! label of the edited revision. Its TP is the number of changed labels, its FP is
//! the number of unchanged labels, its FN is zero by construction, and it explores
//! as many states as the clean run. `subfile_reuses` counts the correct reuses that
//! module granularity would have recomputed. For a one-file corpus, that is every
//! correct reuse. This is the research/27 "sub-file reuse" count, per licence class.
//!
//! # Corpora and modes
//!
//! Two models: the TV-009 Die Hard model with the spike corpus (`support::CORPUS`),
//! and a finite concretization of the durable register (`support::
//! durable_register_finite`, `support::REGISTER_CORPUS`). Each is measured in two
//! lanes (promotion, interactive) and two modes:
//!
//! - **edits**: for each edit, a fresh engine revises the base once (cold, not
//!   counted) and then the edit once (counted).
//! - **trace**: one engine per lane revises the base once (not counted), then every
//!   edit in corpus order, and then the base again, each once (counted).
//!
//! Every revision must hold parity. The one exception is an interactive-lane
//! mismatch whose first divergence is blamed on one reuse. In both modes, when the
//! audit finds such a mismatch whose first divergence is
//! attributed to one reuse, the test quarantines that licence triple and revises the
//! edit again. The re-run must restore parity. It is counted in `quarantine_reruns`
//! only, and in a trace it is the next step's baseline. An inconclusive audit counts
//! only in `inconclusive`. Every count is an exact integer, and every ratio is given
//! as numerator, denominator, and per-mille (rounded down).
//!
//! The test also checks its own classification against `audit::cone`: the FP set
//! must equal `cone.over_invalidated`, and the FN set must equal the union of
//! `cone.under_invalidated` and `cone.experimental_misses`.
//!
//! The report is pinned byte for byte against
//! `tests/golden/pr22a_impl03_precision_baseline.json` (artifact id
//! `pr22a-impl03-precision-baseline`). ADR-0055 records its table. Regenerate with
//! `INCREMENTAL_BLESS=1 cargo test -p continuum-incremental --test precision_baseline`.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use continuum_incremental::audit::{self, AuditVerdict, Blame, CleanRun};
use continuum_incremental::engine::{Cause, Config, Engine, Outcome, Revision};
use continuum_incremental::query::{Compared, DEFAULT_SEMANTIC_EPOCH, Label};
use continuum_incremental::vocab::{Lane, ReuseClass};

use support::{CORPUS, Mutation, REGISTER_CORPUS, diehard, durable_register_finite};

const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/golden/pr22a_impl03_precision_baseline.json"
);

/// A minimal deterministic JSON value: objects keep insertion order.
enum J {
    S(String),
    N(u64),
    A(Vec<J>),
    O(Vec<(String, J)>),
}

fn s(text: impl Into<String>) -> J {
    J::S(text.into())
}

fn o(fields: Vec<(&str, J)>) -> J {
    J::O(fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

fn render(value: &J, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent + 1);
    let close = "  ".repeat(indent);
    match value {
        J::S(text) => {
            out.push('"');
            for c in text.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    c if (c as u32) < 0x20 => {
                        let _ = write!(out, "\\u{:04x}", c as u32);
                    }
                    c => out.push(c),
                }
            }
            out.push('"');
        }
        J::N(n) => {
            let _ = write!(out, "{n}");
        }
        J::A(items) if items.is_empty() => out.push_str("[]"),
        J::A(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                out.push_str(&pad);
                render(item, indent + 1, out);
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            out.push_str(&close);
            out.push(']');
        }
        J::O(fields) if fields.is_empty() => out.push_str("{}"),
        J::O(fields) => {
            out.push_str("{\n");
            for (i, (key, item)) in fields.iter().enumerate() {
                let _ = write!(out, "{pad}\"{key}\": ");
                render(item, indent + 1, out);
                out.push_str(if i + 1 < fields.len() { ",\n" } else { "\n" });
            }
            out.push_str(&close);
            out.push('}');
        }
    }
}

/// A ratio as numerator, denominator and per-mille (rounded down); `none` when the
/// denominator is zero.
fn ratio(num: u64, den: u64) -> J {
    o(vec![
        ("num", J::N(num)),
        ("den", J::N(den)),
        (
            "per_mille",
            (num * 1000)
                .checked_div(den)
                .map_or_else(|| s("none"), J::N),
        ),
    ])
}

/// Reuses through one licence class.
#[derive(Default, Clone, Copy)]
struct ClassCounts {
    /// Compared labels reused through this licence.
    served: u64,
    /// Of those, equal to the clean output (TN).
    correct: u64,
    /// Of those, different from the clean output (FN).
    stale: u64,
    /// Of the stale ones, those whose derivation is `Experimental` although this
    /// licence is not: the licence itself held (same key, same output), and the
    /// staleness came from an `Experimental` reuse upstream.
    stale_downstream_of_experimental: u64,
    /// Clean outputs the engine did not produce, because a stale reuse through this
    /// licence (the audit's first-divergence blame) stopped the pipeline: recall
    /// misses, counted in FN.
    missing_downstream: u64,
}

/// The confusion counts of some audited revisions.
#[derive(Default, Clone)]
struct Stats {
    revisions: u64,
    inconclusive: u64,
    quarantine_reruns: u64,
    judged: u64,
    tp: u64,
    fp: u64,
    fn_strong: u64,
    fn_experimental: u64,
    tn: u64,
    tn_label_changed: u64,
    only_in_engine: u64,
    only_in_clean: u64,
    recompute_wrong: u64,
    by_licence: BTreeMap<ReuseClass, ClassCounts>,
    recompute_by_function: BTreeMap<&'static str, (u64, u64)>,
    fp_by_cause: BTreeMap<&'static str, u64>,
    internal_by_function: BTreeMap<&'static str, (u64, u64)>,
    module_tp: u64,
    module_fp: u64,
    explored_incremental: u64,
    explored_clean: u64,
}

impl Stats {
    fn merge(&mut self, other: &Self) {
        self.revisions += other.revisions;
        self.inconclusive += other.inconclusive;
        self.quarantine_reruns += other.quarantine_reruns;
        self.judged += other.judged;
        self.tp += other.tp;
        self.fp += other.fp;
        self.fn_strong += other.fn_strong;
        self.fn_experimental += other.fn_experimental;
        self.tn += other.tn;
        self.tn_label_changed += other.tn_label_changed;
        self.only_in_engine += other.only_in_engine;
        self.only_in_clean += other.only_in_clean;
        self.recompute_wrong += other.recompute_wrong;
        for (class, c) in &other.by_licence {
            let e = self.by_licence.entry(*class).or_default();
            e.served += c.served;
            e.correct += c.correct;
            e.stale += c.stale;
            e.stale_downstream_of_experimental += c.stale_downstream_of_experimental;
            e.missing_downstream += c.missing_downstream;
        }
        for (f, (a, b)) in &other.recompute_by_function {
            let e = self.recompute_by_function.entry(f).or_default();
            e.0 += a;
            e.1 += b;
        }
        for (cause, n) in &other.fp_by_cause {
            *self.fp_by_cause.entry(cause).or_default() += n;
        }
        for (f, (a, b)) in &other.internal_by_function {
            let e = self.internal_by_function.entry(f).or_default();
            e.0 += a;
            e.1 += b;
        }
        self.module_tp += other.module_tp;
        self.module_fp += other.module_fp;
        self.explored_incremental += other.explored_incremental;
        self.explored_clean += other.explored_clean;
    }

    fn fn_all(&self) -> u64 {
        self.fn_strong + self.fn_experimental
    }

    /// Correct reuses through a strong licence (`Exact`, `Validated`) over every
    /// correct reuse through a non-`Experimental` licence: the R21 collapse measure.
    fn strong_share(&self) -> J {
        let get = |c| {
            self.by_licence
                .get(&c)
                .map_or(0, |x: &ClassCounts| x.correct)
        };
        let strong = get(ReuseClass::Exact) + get(ReuseClass::Validated);
        ratio(strong, strong + get(ReuseClass::Conservative))
    }

    fn json(&self) -> J {
        let by_licence = J::O(
            ReuseClass::ALL
                .iter()
                .map(|class| {
                    let c = self.by_licence.get(class).copied().unwrap_or_default();
                    (
                        class.token().to_owned(),
                        o(vec![
                            ("served", J::N(c.served)),
                            ("correct", J::N(c.correct)),
                            ("stale", J::N(c.stale)),
                            (
                                "stale_downstream_of_experimental",
                                J::N(c.stale_downstream_of_experimental),
                            ),
                            ("missing_downstream", J::N(c.missing_downstream)),
                            ("reuse_precision", ratio(c.correct, c.served)),
                            ("subfile_reuses", J::N(c.correct)),
                        ]),
                    )
                })
                .collect(),
        );
        let recompute = J::O(
            self.recompute_by_function
                .iter()
                .map(|(f, (need, over))| {
                    (
                        (*f).to_owned(),
                        o(vec![("necessary", J::N(*need)), ("over", J::N(*over))]),
                    )
                })
                .collect(),
        );
        let internal = J::O(
            self.internal_by_function
                .iter()
                .map(|(f, (reused, recomputed))| {
                    (
                        (*f).to_owned(),
                        o(vec![
                            ("reused", J::N(*reused)),
                            ("recomputed", J::N(*recomputed)),
                        ]),
                    )
                })
                .collect(),
        );
        o(vec![
            ("counted_revisions", J::N(self.revisions)),
            ("inconclusive", J::N(self.inconclusive)),
            ("quarantine_reruns", J::N(self.quarantine_reruns)),
            ("judged_labels", J::N(self.judged)),
            (
                "spike",
                o(vec![
                    ("tp_necessary_recompute", J::N(self.tp)),
                    ("fp_over_invalidated", J::N(self.fp)),
                    ("fn_under_invalidated", J::N(self.fn_strong)),
                    ("fn_experimental_misses", J::N(self.fn_experimental)),
                    ("tn_correct_reuse", J::N(self.tn)),
                    ("tn_label_changed", J::N(self.tn_label_changed)),
                    ("only_in_engine", J::N(self.only_in_engine)),
                    ("fn_only_in_clean", J::N(self.only_in_clean)),
                    ("recompute_wrong", J::N(self.recompute_wrong)),
                    ("precision", ratio(self.tp, self.tp + self.fp)),
                    ("recall", ratio(self.tp, self.tp + self.fn_all())),
                    ("recompute_share", ratio(self.tp + self.fp, self.judged)),
                    ("strong_share_of_sound_reuse", self.strong_share()),
                    ("explored_states", J::N(self.explored_incremental)),
                ]),
            ),
            (
                "module_granularity",
                o(vec![
                    ("tp_necessary_recompute", J::N(self.module_tp)),
                    ("fp_over_invalidated", J::N(self.module_fp)),
                    ("fn_under_invalidated", J::N(0)),
                    (
                        "precision",
                        ratio(self.module_tp, self.module_tp + self.module_fp),
                    ),
                    ("recall", ratio(self.module_tp, self.module_tp)),
                    ("labels", J::N(self.module_tp + self.module_fp)),
                    ("explored_states", J::N(self.explored_clean)),
                ]),
            ),
            ("by_licence", by_licence),
            ("recompute_by_function", recompute),
            (
                "fp_by_cause",
                J::O(
                    self.fp_by_cause
                        .iter()
                        .map(|(cause, n)| ((*cause).to_owned(), J::N(*n)))
                        .collect(),
                ),
            ),
            ("internal_unjudged", internal),
        ])
    }
}

/// The report token of a recompute cause.
const fn cause_token(cause: &Cause) -> &'static str {
    match cause {
        Cause::KeyMiss => "key-miss",
        Cause::LaneRefused(_) => "lane-refused",
        Cause::Quarantined(_) => "quarantined",
        Cause::WitnessRefused(_) => "witness-refused",
        Cause::AlternateRefused(_) => "alternate-refused",
    }
}

/// Judge one audited revision. Returns its stats and a one-line summary.
fn judge(
    before_clean: &CleanRun,
    after_clean: &CleanRun,
    before_revision: &Revision,
    revision: &Revision,
) -> (Stats, String) {
    let mut st = Stats {
        revisions: 1,
        ..Stats::default()
    };
    let report = audit::parity(revision, after_clean);
    let cone = audit::cone(before_clean, after_clean, before_revision, revision);
    if report.verdict() == AuditVerdict::Inconclusive || !cone.incomplete.is_empty() {
        st.inconclusive = 1;
        return (st, "inconclusive".to_owned());
    }
    st.explored_incremental = revision.explored_states;
    st.explored_clean = after_clean.explored_states();

    let before = before_clean.outputs();
    let after = after_clean.outputs();
    let mut fp_set = BTreeSet::new();
    let mut fn_set = BTreeSet::new();
    let mut over_names = Vec::new();
    let mut phantom_reused = BTreeSet::new();
    for (label, record) in &revision.records {
        let function = record.definition.function_id;
        if label.stage.compared() != Compared::Yes {
            let e = st.internal_by_function.entry(function).or_default();
            match record.outcome {
                Outcome::Reused { .. } => e.0 += 1,
                Outcome::Recomputed { .. } => e.1 += 1,
            }
            continue;
        }
        let clean_before = before.get(label);
        let clean_after = after.get(label);
        // A label the clean run of the edited source does not define is a phantom
        // artifact, not a missed invalidation: counted apart, outside the confusion
        // counts and recall.
        if clean_after.is_none() {
            st.only_in_engine += 1;
            if matches!(record.outcome, Outcome::Reused { .. }) {
                phantom_reused.insert(label.clone());
            }
            continue;
        }
        st.judged += 1;
        let changed = clean_before.is_none() || clean_before != clean_after;
        match &record.outcome {
            Outcome::Reused { licence, .. } => {
                let c = st.by_licence.entry(*licence).or_default();
                c.served += 1;
                if clean_after.is_some_and(|b| b.as_slice() == record.output.bytes()) {
                    c.correct += 1;
                    st.tn += 1;
                    if changed {
                        st.tn_label_changed += 1;
                    }
                } else {
                    c.stale += 1;
                    if record.class == ReuseClass::Experimental
                        && *licence != ReuseClass::Experimental
                    {
                        c.stale_downstream_of_experimental += 1;
                    }
                    fn_set.insert(label.clone());
                    // The same split `audit::cone` makes: only the interactive lane
                    // serves `Experimental` derivations.
                    if record.class == ReuseClass::Experimental
                        && revision.lane == Lane::Interactive
                    {
                        st.fn_experimental += 1;
                    } else {
                        st.fn_strong += 1;
                    }
                }
            }
            Outcome::Recomputed { cause, .. } => {
                // A recompute is judged too: its output must be the clean one.
                if clean_after.is_none_or(|b| b.as_slice() != record.output.bytes()) {
                    st.recompute_wrong += 1;
                }
                let e = st.recompute_by_function.entry(function).or_default();
                if changed {
                    st.tp += 1;
                    e.0 += 1;
                } else {
                    st.fp += 1;
                    e.1 += 1;
                    fp_set.insert(label.clone());
                    *st.fp_by_cause.entry(cause_token(cause)).or_default() += 1;
                    over_names.push(label.to_string());
                }
            }
        }
    }
    // A clean output the engine never produced is a recall miss: an under-
    // invalidation of everything downstream of a stale reuse. It is attributed to
    // the licence the audit blames for the first divergence, and counted in FN.
    let blamed = match &report.first {
        Some((_, Blame::Reuse(triple))) => Some(triple.class),
        _ => None,
    };
    let mut missing_names = Vec::new();
    for label in after.keys() {
        if revision.records.contains_key(label) {
            continue;
        }
        st.only_in_clean += 1;
        missing_names.push(label.to_string());
        match blamed {
            Some(class) => {
                st.by_licence.entry(class).or_default().missing_downstream += 1;
                if class == ReuseClass::Experimental && revision.lane == Lane::Interactive {
                    st.fn_experimental += 1;
                } else {
                    st.fn_strong += 1;
                }
            }
            None => st.fn_strong += 1,
        }
    }
    assert_eq!(
        st.judged + st.only_in_clean,
        after.len() as u64,
        "every clean label is judged or counted as missing"
    );
    // Module granularity recomputes every compared label the edited source defines.
    for label in after.keys() {
        if before.get(label) == after.get(label) {
            st.module_fp += 1;
        } else {
            st.module_tp += 1;
        }
    }

    // The classification agrees with the audit's own cone check.
    let cone_fp: BTreeSet<Label> = cone.over_invalidated.iter().cloned().collect();
    assert_eq!(
        fp_set, cone_fp,
        "over-invalidation disagrees with audit::cone"
    );
    let cone_fn: BTreeSet<Label> = cone
        .under_invalidated
        .iter()
        .chain(&cone.experimental_misses)
        .cloned()
        .collect();
    let ours: BTreeSet<Label> = fn_set.union(&phantom_reused).cloned().collect();
    assert_eq!(ours, cone_fn, "stale reuse disagrees with audit::cone");

    let verdict = match report.verdict() {
        AuditVerdict::Parity => "parity",
        AuditVerdict::Mismatch => "mismatch",
        AuditVerdict::Inconclusive => "inconclusive",
    };
    let mut line = format!(
        "{verdict} | TP {} FP {} FN {} TN {} | module TP {} FP {}",
        st.tp,
        st.fp,
        st.fn_all(),
        st.tn,
        st.module_tp,
        st.module_fp
    );
    if st.only_in_engine > 0 {
        let _ = write!(line, " | only in engine {}", st.only_in_engine);
    }
    if !missing_names.is_empty() {
        let _ = write!(line, " | only in clean: {}", missing_names.join(" "));
    }
    if !over_names.is_empty() {
        let _ = write!(line, " | over: {}", over_names.join(" "));
    }
    (st, line)
}

/// Revise `after` on `engine`, judge it, and apply the quarantine rule. Returns the
/// stats, the summary line, and the revision and clean run the next step starts
/// from.
fn step(
    engine: &mut Engine,
    before_revision: &Revision,
    before_clean: &CleanRun,
    after: &str,
    lane: Lane,
) -> (Stats, String, Revision, CleanRun) {
    let limits = engine.config().limits;
    let revision = engine.revise(after, lane).expect("the corpus fits");
    let after_clean = audit::clean(after, &limits, DEFAULT_SEMANTIC_EPOCH);
    let (mut st, mut line) = judge(before_clean, &after_clean, before_revision, &revision);
    let report = audit::parity(&revision, &after_clean);
    let mut next = revision;
    // Every revision holds parity, or its first divergence is blamed on one reuse
    // and the quarantine re-run restores parity. The promotion lane holds parity
    // outright. Nothing else is admitted: a mismatch blamed on a recompute, or an
    // ambiguous blame, fails here rather than being pinned by a re-bless.
    match (report.verdict(), &report.first) {
        (AuditVerdict::Parity, _) => {}
        (AuditVerdict::Mismatch, Some((_, Blame::Reuse(_)))) if lane == Lane::Interactive => {}
        (verdict, first) => panic!("{lane:?}: audit {verdict:?}, first divergence {first:?}"),
    }
    if let Some((_, Blame::Reuse(triple))) = &report.first {
        engine.quarantine(*triple);
        let rerun = engine.revise(after, lane).expect("the corpus fits");
        assert_eq!(
            audit::parity(&rerun, &after_clean).verdict(),
            AuditVerdict::Parity,
            "quarantining {triple} must restore parity"
        );
        st.quarantine_reruns += 1;
        let _ = write!(line, " | quarantined {triple}");
        next = rerun;
    }
    (st, line, next, after_clean)
}

fn edits_mode(base: &str, corpus: &[Mutation], lane: Lane) -> (Stats, Vec<J>) {
    let limits = Config::default().limits;
    let base_clean = audit::clean(base, &limits, DEFAULT_SEMANTIC_EPOCH);
    let mut total = Stats::default();
    let mut lines = Vec::new();
    for mutation in corpus {
        let mut engine = Engine::new(Config::default());
        let cold = engine.revise(base, lane).expect("the base fits");
        assert_eq!(
            audit::parity(&cold, &base_clean).verdict(),
            AuditVerdict::Parity,
            "cold revision of the base"
        );
        let edited = mutation.apply(base);
        let (st, line, _, _) = step(&mut engine, &cold, &base_clean, &edited, lane);
        total.merge(&st);
        lines.push(s(format!("{}: {line}", mutation.id)));
    }
    (total, lines)
}

fn trace_mode(base: &str, corpus: &[Mutation], lane: Lane) -> (Stats, Vec<J>) {
    let limits = Config::default().limits;
    let mut engine = Engine::new(Config::default());
    let mut revision = engine.revise(base, lane).expect("the base fits");
    let mut clean = audit::clean(base, &limits, DEFAULT_SEMANTIC_EPOCH);
    let mut total = Stats::default();
    let mut lines = Vec::new();
    let sources = corpus
        .iter()
        .map(|m| (m.id, m.apply(base)))
        .chain(std::iter::once(("base", base.to_owned())));
    for (id, source) in sources {
        let (st, line, next, next_clean) = step(&mut engine, &revision, &clean, &source, lane);
        total.merge(&st);
        lines.push(s(format!("{id}: {line}")));
        revision = next;
        clean = next_clean;
    }
    (total, lines)
}

/// Every measured stats block, keyed `(corpus, mode, lane)`.
type Measured = BTreeMap<(&'static str, &'static str, &'static str), Stats>;

fn build_report() -> (String, Measured) {
    let corpora: [(&str, &str, String, &[Mutation]); 2] = [
        (
            "diehard",
            "notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm",
            diehard(),
            CORPUS,
        ),
        (
            "durable_register_finite",
            "crates/continuum-incremental/tests/fixtures/durable_register_finite.ctm",
            durable_register_finite(),
            REGISTER_CORPUS,
        ),
    ];
    let lanes = [Lane::Promotion, Lane::Interactive];
    let mut measured = Measured::new();
    let mut combined: BTreeMap<&str, Stats> = BTreeMap::new();
    let mut corpus_json = Vec::new();
    for (name, model, base, corpus) in &corpora {
        let mut edits = Vec::new();
        let mut traces = Vec::new();
        let mut cases = Vec::new();
        let mut trace_cases = Vec::new();
        for lane in lanes {
            let (st, lines) = edits_mode(base, corpus, lane);
            combined.entry(lane.token()).or_default().merge(&st);
            edits.push((lane.token().to_owned(), st.json()));
            cases.push((lane.token().to_owned(), J::A(lines)));
            measured.insert((name, "edits", lane.token()), st);
            let (st, lines) = trace_mode(base, corpus, lane);
            traces.push((lane.token().to_owned(), st.json()));
            trace_cases.push((lane.token().to_owned(), J::A(lines)));
            measured.insert((name, "trace", lane.token()), st);
        }
        corpus_json.push((
            (*name).to_owned(),
            o(vec![
                ("model", s(*model)),
                ("edits", J::N(corpus.len() as u64)),
                ("edits_mode", J::O(edits)),
                ("trace_mode", J::O(traces)),
                ("cases", J::O(cases)),
                ("trace_cases", J::O(trace_cases)),
            ]),
        ));
    }
    let report = o(vec![
        ("artifact_id", s("pr22a-impl03-precision-baseline")),
        ("schema_version", J::N(1)),
        ("bone", s("bn-3gf6")),
        ("requirement", s("PR-22A-IMPL-03")),
        (
            "adr",
            s("notes/plan/adr/0055-incremental-engine-build-vs-adopt.md"),
        ),
        (
            "spec",
            s("notes/plan/rfcs/0030-incremental-semantic-query-engine.md"),
        ),
        (
            "definitions",
            s(
                "unit: one compared label (parse, elaborate, explore, domain_safety, check_invariant) of one audited revision before->after. changed(L): the clean output of L for before is absent or differs from the clean output for after. TP: recomputed and changed. FP (over-invalidation): recomputed, and the clean output exists before and is equal after. FN (under-invalidation): reused, and the reused output differs from the clean output after; FN also counts every clean output the engine did not produce (fn_only_in_clean; a stale reuse stopped the pipeline), attributed to the licence the audit blames for the first divergence (by_licence missing_downstream). fn_experimental_misses are the FN on an Experimental derivation or blame in the interactive lane, fn_under_invalidated all others. judged_labels + fn_only_in_clean = module_granularity.labels (asserted). TN: reused and equal to the clean output after; tn_label_changed: TN where changed(L) also holds (content-addressed reuse across a label change). precision = TP/(TP+FP); recall = TP/(TP+FN). recompute_share = (TP+FP)/judged_labels. only_in_engine: labels the engine produced that the clean run of the edited source does not define (phantom artifacts, outside TP/FP/FN/TN and recall). recompute_wrong: recomputed labels whose output differs from the clean output (asserted 0). Every revision holds parity, except an interactive-lane mismatch blamed on one reuse, whose quarantine re-run must hold parity. by_licence stale_downstream_of_experimental: stale reuses whose own licence held but whose derivation is Experimental (the staleness came from an Experimental reuse upstream). strong_share_of_sound_reuse = correct Exact+Validated reuses / correct Exact+Validated+Conservative reuses (docs/08 R21: Conservative-collapse drives this toward 0). module_granularity: every compared label of the edited revision is recomputed, because each corpus model is one file; labels = its clean label count; FN = 0 by construction. subfile_reuses: correct reuses that module granularity would have recomputed (research/27). fp_by_cause: the engine's recompute cause of each FP (key-miss: some keyed input identity changed although this output did not). internal_unjudged: build_model and the projections have no clean counterpart and are counted only. Ratios: per_mille is rounded down; none when den = 0.",
            ),
        ),
        (
            "method",
            s(
                "edits_mode: for each edit, a fresh engine revises the base once (cold, not counted) and then the edit once (counted). trace_mode: one engine per lane revises the base once (not counted), then every edit in corpus order and the base again, each once (counted). In both, a Mismatch whose first divergence is attributed to one reuse (Blame::Reuse) quarantines that licence triple and revises the edit again; the re-run must restore parity, counts only in quarantine_reruns, and in a trace is the next step's baseline. An Inconclusive audit counts only in inconclusive. The FP and FN sets are asserted equal to audit::cone's over_invalidated and under_invalidated + experimental_misses.",
            ),
        ),
        ("corpora", J::O(corpus_json)),
        (
            "combined_edits_mode",
            J::O(
                lanes
                    .iter()
                    .map(|lane| (lane.token().to_owned(), combined[lane.token()].json()))
                    .collect(),
            ),
        ),
    ]);
    for lane in lanes {
        measured.insert(
            ("combined", "edits", lane.token()),
            combined[lane.token()].clone(),
        );
    }
    let mut out = String::new();
    render(&report, 0, &mut out);
    out.push('\n');
    (out, measured)
}

#[test]
fn the_retained_precision_baseline_is_current_and_sound() {
    let (report, measured) = build_report();
    if let Some(path) = std::env::var_os("INCREMENTAL_DUMP") {
        std::fs::write(path, &report).expect("write the dump");
    }

    for ((corpus, mode, lane), st) in &measured {
        let at = format!("{corpus}/{mode}/{lane}");
        assert_eq!(st.fn_strong, 0, "{at}: under-invalidation");
        assert_eq!(
            st.judged + st.only_in_clean,
            st.module_tp + st.module_fp,
            "{at}: every clean label is judged or missing"
        );
        assert_eq!(st.inconclusive, 0, "{at}: every corpus run completes");
        assert_eq!(
            st.recompute_wrong, 0,
            "{at}: a recompute differs from clean"
        );
        // Module granularity recomputes at least what the spike does.
        assert!(
            st.module_tp + st.module_fp >= st.tp + st.fp,
            "{at}: the spike recomputed more than module granularity"
        );
        if *lane == "promotion" {
            assert_eq!(st.fn_experimental, 0, "{at}: no Experimental in promotion");
            assert_eq!(
                st.only_in_engine, 0,
                "{at}: no phantom artifact in promotion"
            );
            assert_eq!(st.quarantine_reruns, 0, "{at}: no quarantine in promotion");
            assert!(
                st.by_licence
                    .get(&ReuseClass::Experimental)
                    .is_none_or(|c| c.served == 0),
                "{at}: the promotion lane admits no Experimental reuse"
            );
        }
    }
    // Each corpus exercises the strong and conservative classes in promotion.
    for corpus in ["diehard", "durable_register_finite"] {
        let st = &measured[&(corpus, "edits", "promotion")];
        for class in [
            ReuseClass::Exact,
            ReuseClass::Validated,
            ReuseClass::Conservative,
        ] {
            assert!(
                st.by_licence.get(&class).is_some_and(|c| c.correct > 0),
                "{corpus}: the corpus exercises a correct {class} reuse"
            );
        }
        assert!(st.tp > 0 && st.module_fp > 0, "{corpus}: non-vacuous");
    }

    if std::env::var_os("INCREMENTAL_BLESS").is_some() {
        std::fs::write(GOLDEN, &report).expect("write the golden report");
    }
    let golden = std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|e| panic!("{GOLDEN}: {e} (regenerate with INCREMENTAL_BLESS=1)"));
    assert!(
        golden == report,
        "the retained baseline is stale; regenerate with INCREMENTAL_BLESS=1 and review the diff"
    );
}
