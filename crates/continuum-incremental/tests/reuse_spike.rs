//! PR-22A / IMPL-02 (bn-31vf): the reuse-edge spike over the CML mutation corpus,
//! with its retained evidence.
//!
//! For each lane (promotion, interactive) and each corpus edit of the TV-009 Die
//! Hard model, a warm engine revises from the base to the edit, and the revision is
//! audited against the independent clean recomputation of both sources:
//!
//! - **parity** (INV-010): every compared artifact is byte-equal to the clean one;
//! - **cone soundness**: no query whose clean output changed was reused through an
//!   `Exact`, `Validated` or `Conservative` licence (under-invalidation);
//! - a divergence is attributed to its first divergent query's licence triple, the
//!   triple is quarantined, and the edit is revised again to show the downgrade
//!   forces a recompute and restores parity.
//!
//! A trace then walks every edit in sequence on one engine per lane.
//!
//! The report is deterministic and is pinned byte for byte against
//! `tests/golden/pr22a_impl02_reuse_spike.json` (artifact id
//! `pr22a-impl02-reuse-spike`). Regenerate with
//! `INCREMENTAL_BLESS=1 cargo test -p continuum-incremental --test reuse_spike`.

mod support;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use continuum_incremental::audit::{self, AuditVerdict, Blame, CleanRun};
use continuum_incremental::engine::{Config, Engine, Memo, Outcome, Revision};
use continuum_incremental::explain::{self, Cause, Invalidation};
use continuum_incremental::query::Compared;
use continuum_incremental::vocab::{Lane, ReuseClass};

use support::{CORPUS, diehard};

const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/golden/pr22a_impl02_reuse_spike.json"
);

/// A tiny deterministic JSON writer: objects keep insertion order, which this file
/// fixes; nothing is hashed.
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

/// Per-class and per-outcome counts over some revisions.
#[derive(Default, Clone)]
struct Counts {
    reuse: BTreeMap<ReuseClass, u64>,
    derived: BTreeMap<ReuseClass, u64>,
    recomputed: u64,
    not_memoized: u64,
    queries: u64,
    compared: u64,
    mismatches: u64,
    inconclusive: u64,
    under_invalidated: u64,
    experimental_misses: u64,
    over_invalidated: u64,
    true_cone: u64,
    computed_cone: u64,
    explored_incremental: u64,
    explored_clean: u64,
    cases: u64,
    revisions: u64,
    quarantine_reruns: u64,
}

impl Counts {
    fn add_revision(&mut self, revision: &Revision) {
        for record in revision.records.values() {
            self.queries += 1;
            *self.derived.entry(record.class).or_default() += 1;
            if record.label.stage.compared() == Compared::Yes {
                self.compared += 1;
            }
            match &record.outcome {
                Outcome::Reused { licence, .. } => *self.reuse.entry(*licence).or_default() += 1,
                Outcome::Recomputed { memo, .. } => {
                    self.recomputed += 1;
                    if matches!(memo, Memo::NotMemoized(_)) {
                        self.not_memoized += 1;
                    }
                }
            }
        }
        self.explored_incremental += revision.explored_states;
    }

    fn merge(&mut self, other: &Self) {
        for (class, n) in &other.reuse {
            *self.reuse.entry(*class).or_default() += n;
        }
        for (class, n) in &other.derived {
            *self.derived.entry(*class).or_default() += n;
        }
        self.recomputed += other.recomputed;
        self.not_memoized += other.not_memoized;
        self.queries += other.queries;
        self.compared += other.compared;
        self.mismatches += other.mismatches;
        self.inconclusive += other.inconclusive;
        self.under_invalidated += other.under_invalidated;
        self.experimental_misses += other.experimental_misses;
        self.over_invalidated += other.over_invalidated;
        self.true_cone += other.true_cone;
        self.computed_cone += other.computed_cone;
        self.explored_incremental += other.explored_incremental;
        self.explored_clean += other.explored_clean;
        self.cases += other.cases;
        self.revisions += other.revisions;
        self.quarantine_reruns += other.quarantine_reruns;
    }

    fn json(&self) -> J {
        let classes = |map: &BTreeMap<ReuseClass, u64>| {
            J::O(
                ReuseClass::ALL
                    .iter()
                    .map(|c| (c.token().to_owned(), J::N(map.get(c).copied().unwrap_or(0))))
                    .collect(),
            )
        };
        o(vec![
            ("counted_revisions", J::N(self.revisions)),
            ("quarantine_reruns", J::N(self.quarantine_reruns)),
            ("queries", J::N(self.queries)),
            ("reused_by_licence", classes(&self.reuse)),
            ("records_by_derivation_class", classes(&self.derived)),
            ("recomputed", J::N(self.recomputed)),
            ("not_memoized", J::N(self.not_memoized)),
            ("compared", J::N(self.compared)),
            ("parity_mismatches", J::N(self.mismatches)),
            ("parity_inconclusive_runs", J::N(self.inconclusive)),
            ("true_cone", J::N(self.true_cone)),
            ("computed_cone", J::N(self.computed_cone)),
            ("under_invalidated", J::N(self.under_invalidated)),
            ("experimental_misses", J::N(self.experimental_misses)),
            ("over_invalidated", J::N(self.over_invalidated)),
            (
                "explored_states_incremental",
                J::N(self.explored_incremental),
            ),
            ("explored_states_clean", J::N(self.explored_clean)),
        ])
    }
}

fn chain_text(cause: &Cause) -> String {
    match cause {
        Cause::Input(steps) => {
            let mut text = steps
                .first()
                .map(|step| step.from.to_string())
                .unwrap_or_default();
            for step in steps {
                let _ = write!(text, " -{}/{}-> {}", step.reason, step.class, step.to);
            }
            text
        }
        Cause::Downgrade(cause) => format!("downgrade: {cause:?}"),
        Cause::NotCached => "not-cached".to_owned(),
        Cause::NotMemoized(reason) => format!("not-memoized: {reason}"),
    }
}

fn explain_json(invalidation: &Invalidation) -> J {
    o(vec![
        (
            "invalidated",
            J::A(
                invalidation
                    .invalidated
                    .iter()
                    .map(|entry| {
                        s(format!(
                            "{} [{:?}] {}",
                            entry.label,
                            entry.status,
                            chain_text(&entry.cause)
                        ))
                    })
                    .collect(),
            ),
        ),
        (
            "unknown",
            J::A(
                invalidation
                    .unknown
                    .iter()
                    .map(|entry| s(entry.label.to_string()))
                    .collect(),
            ),
        ),
        (
            "reused",
            J::A(
                invalidation
                    .reused
                    .iter()
                    .map(|entry| {
                        let mut text = format!(
                            "{} licence={} evidence={} class={}",
                            entry.label,
                            entry.licence,
                            entry.evidence.token(),
                            entry.class
                        );
                        for assumption in &entry.assumptions {
                            let tag = assumption.split(':').next().unwrap_or("");
                            let _ = write!(text, " assumes={tag}");
                        }
                        s(text)
                    })
                    .collect(),
            ),
        ),
    ])
}

/// Audit one warm revision; returns the case report and its counts.
fn audit_case(
    engine: &mut Engine,
    before_revision: &Revision,
    before_clean: &CleanRun,
    after_source: &str,
    lane: Lane,
) -> (Vec<(&'static str, J)>, Counts, Revision, CleanRun) {
    let limits = engine.config().limits;
    let revision = engine
        .revise(after_source, lane)
        .expect("the corpus fits the limits");
    let after_clean = audit::clean(
        after_source,
        &limits,
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    let report = audit::parity(&revision, &after_clean);
    let cone = audit::cone(before_clean, &after_clean, before_revision, &revision);
    let invalidation = explain::explain_invalidation(before_revision, &revision);

    let mut counts = Counts::default();
    counts.add_revision(&revision);
    counts.revisions += 1;
    counts.explored_clean += after_clean.explored_states();
    // An inconclusive audit counts as neither parity nor mismatch.
    match report.verdict() {
        AuditVerdict::Mismatch => counts.mismatches += report.mismatches.len() as u64,
        AuditVerdict::Inconclusive => counts.inconclusive += 1,
        AuditVerdict::Parity => {}
    }
    // Cone counts only for a judged audit: an inconclusive one is neither sound
    // nor unsound.
    if report.verdict() != AuditVerdict::Inconclusive && cone.incomplete.is_empty() {
        counts.under_invalidated += cone.under_invalidated.len() as u64;
        counts.experimental_misses += cone.experimental_misses.len() as u64;
        counts.over_invalidated += cone.over_invalidated.len() as u64;
        counts.true_cone += cone.true_cone.len() as u64;
        counts.computed_cone += cone.computed_cone.len() as u64;
    }
    counts.cases += 1;

    let labels = |v: &[continuum_incremental::query::Label]| {
        J::A(v.iter().map(|l| s(l.to_string())).collect())
    };
    let mut fields = vec![
        (
            "parity",
            s(match report.verdict() {
                AuditVerdict::Parity => "parity",
                AuditVerdict::Mismatch => "mismatch",
                AuditVerdict::Inconclusive => "inconclusive",
            }),
        ),
        (
            "mismatches",
            J::A(
                report
                    .mismatches
                    .iter()
                    .map(|m| s(format!("{} {:?} class={:?}", m.label, m.parity, m.class)))
                    .collect(),
            ),
        ),
        ("under_invalidated", labels(&cone.under_invalidated)),
        ("experimental_misses", labels(&cone.experimental_misses)),
        ("over_invalidated", labels(&cone.over_invalidated)),
        ("counts", counts.json()),
        ("explain", explain_json(&invalidation)),
    ];

    // Attribution, quarantine, and the re-run the downgrade forces. The re-run is
    // the one extra revision of an edit: it is reported under `after_quarantine`,
    // counted in `quarantine_reruns` (never in the reuse totals), and it is the
    // revision the next trace step starts from.
    let mut revision = revision;
    if let Some((label, blame)) = &report.first {
        fields.push(("first_divergent", s(label.to_string())));
        match blame {
            Blame::Reuse(triple) => {
                fields.push(("quarantined", s(triple.to_string())));
                engine.quarantine(*triple);
                let rerun = engine
                    .revise(after_source, lane)
                    .expect("the corpus fits the limits");
                let rerun_report = audit::parity(&rerun, &after_clean);
                assert_eq!(
                    rerun_report.verdict(),
                    AuditVerdict::Parity,
                    "quarantining {triple} must restore parity"
                );
                let mut rerun_counts = Counts::default();
                rerun_counts.add_revision(&rerun);
                rerun_counts.revisions = 1;
                counts.quarantine_reruns += 1;
                revision = rerun;
                fields.push((
                    "after_quarantine",
                    o(vec![
                        ("parity", s("parity")),
                        ("counts", rerun_counts.json()),
                    ]),
                ));
            }
            Blame::Ambiguous(triples) => {
                fields.push((
                    "ambiguous",
                    J::A(triples.iter().map(|t| s(t.to_string())).collect()),
                ));
            }
            Blame::Recompute(label) => {
                fields.push(("engine_defect", s(label.to_string())));
            }
        }
    }
    (fields, counts, revision, after_clean)
}

fn lane_report(lane: Lane) -> (J, Counts) {
    let base = diehard();
    let limits = Config::default().limits;
    let base_clean = audit::clean(
        &base,
        &limits,
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    let mut cases = Vec::new();
    let mut totals = Counts::default();
    for mutation in CORPUS {
        let mut engine = Engine::new(Config::default());
        let cold = engine.revise(&base, lane).expect("the base fits");
        assert_eq!(
            audit::parity(&cold, &base_clean).verdict(),
            AuditVerdict::Parity,
            "cold revision of the base"
        );
        let edited = mutation.apply(&base);
        let (mut fields, counts, _, _) = audit_case(&mut engine, &cold, &base_clean, &edited, lane);
        totals.merge(&counts);
        let mut case = vec![
            ("id", s(mutation.id)),
            ("kind", s(mutation.kind)),
            ("description", s(mutation.description)),
        ];
        case.append(&mut fields);
        cases.push(o(case));
    }
    (
        o(vec![("cases", J::A(cases)), ("totals", totals.json())]),
        totals,
    )
}

fn trace_report(lane: Lane) -> (J, Counts) {
    let base = diehard();
    let limits = Config::default().limits;
    let mut engine = Engine::new(Config::default());
    let mut previous_source = base.clone();
    let mut previous_revision = engine.revise(&base, lane).expect("the base fits");
    let mut previous_clean = audit::clean(
        &base,
        &limits,
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    let mut totals = Counts::default();
    let mut steps = Vec::new();
    let sources: Vec<(&str, String)> = CORPUS
        .iter()
        .map(|m| (m.id, m.apply(&base)))
        .chain(std::iter::once(("base", base.clone())))
        .collect();
    for (id, source) in sources {
        assert_ne!(source, previous_source);
        // One revision per edit: the audited revision and its clean run are the
        // next step's baseline; nothing is revised twice.
        let (fields, counts, revision, clean) = audit_case(
            &mut engine,
            &previous_revision,
            &previous_clean,
            &source,
            lane,
        );
        totals.merge(&counts);
        let parity = fields
            .iter()
            .find(|(k, _)| *k == "parity")
            .map(|(_, v)| match v {
                J::S(text) => text.clone(),
                _ => String::new(),
            })
            .unwrap_or_default();
        let quarantined = fields
            .iter()
            .find(|(k, _)| *k == "quarantined")
            .map(|(_, v)| match v {
                J::S(text) => text.clone(),
                _ => String::new(),
            });
        let mut text = format!("{id}: {parity}");
        if let Some(triple) = quarantined {
            let _ = write!(text, ", quarantined {triple}");
        }
        steps.push(s(text));
        previous_revision = revision;
        previous_clean = clean;
        previous_source = source;
    }
    (
        o(vec![
            ("steps", J::A(steps)),
            (
                "quarantined_at_end",
                J::A(
                    engine
                        .quarantined()
                        .iter()
                        .map(|t| s(t.to_string()))
                        .collect(),
                ),
            ),
            ("totals", totals.json()),
        ]),
        totals,
    )
}

fn build_report() -> (
    String,
    BTreeMap<&'static str, Counts>,
    BTreeMap<&'static str, Counts>,
) {
    let mut lanes = Vec::new();
    let mut traces = Vec::new();
    let mut all = BTreeMap::new();
    let mut trace_counts = BTreeMap::new();
    for lane in [Lane::Promotion, Lane::Interactive] {
        let (json, counts) = lane_report(lane);
        lanes.push((lane.token().to_owned(), json));
        all.insert(lane.token(), counts);
        let (json, counts) = trace_report(lane);
        traces.push((lane.token().to_owned(), json));
        trace_counts.insert(lane.token(), counts);
    }
    let report = o(vec![
        ("artifact_id", s("pr22a-impl02-reuse-spike")),
        ("schema_version", J::N(1)),
        ("bone", s("bn-31vf")),
        ("requirement", s("PR-22A-IMPL-02")),
        (
            "spec",
            s("notes/plan/rfcs/0030-incremental-semantic-query-engine.md"),
        ),
        (
            "model",
            s("notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm"),
        ),
        (
            "method",
            s(
                "lanes: for each edit, a fresh engine revises the base once (cold, not counted) and then the edit once (counted_revisions); that revision is compared with an independent clean recomputation of the edit (crate::audit::clean, which derives its own basis), and its invalidation cone with the clean runs of base and edit. traces: one engine per lane revises the base once (not counted), then every edit in corpus order and the base again, each exactly once (counted_revisions). In both, a Mismatch whose first divergence is attributed to one reuse (Blame::Reuse) quarantines that licence triple and revises the edit one more time (quarantine_reruns; reported under after_quarantine, whose counted_revisions is that one re-run, and excluded from the totals); an ambiguous or recompute attribution quarantines nothing and re-runs nothing. In a trace the re-run is the next step's baseline. An Inconclusive audit counts only in parity_inconclusive_runs. Counts are deterministic: revisions, queries, reuses by licence class, records by derivation class, states explored.",
            ),
        ),
        (
            "baseline_module_granularity",
            s(
                "module-granularity invalidation recomputes every query of a changed file: its explored states equal explored_states_clean, and its reuse is zero",
            ),
        ),
        ("lanes", J::O(lanes)),
        ("traces", J::O(traces)),
    ]);
    let mut out = String::new();
    render(&report, 0, &mut out);
    out.push('\n');
    (out, all, trace_counts)
}

#[test]
fn the_retained_spike_report_is_current_and_sound() {
    let (report, lanes, traces) = build_report();
    if let Some(path) = std::env::var_os("INCREMENTAL_DUMP") {
        std::fs::write(path, &report).expect("write the dump");
    }

    // The soundness facts the report records, asserted rather than only written.
    for (lane, counts) in &lanes {
        assert_eq!(
            counts.under_invalidated, 0,
            "{lane}: a strong or conservative reuse missed a changed artifact"
        );
    }
    for (lane, counts) in &lanes {
        assert_eq!(
            counts.inconclusive, 0,
            "{lane}: every corpus run completes within bounds"
        );
        assert_eq!(
            counts.revisions,
            CORPUS.len() as u64,
            "{lane}: one counted revision per edit"
        );
    }
    for (lane, counts) in &traces {
        assert_eq!(
            counts.revisions,
            CORPUS.len() as u64 + 1,
            "{lane} trace: each edit and the final base revised exactly once"
        );
    }
    let promotion = &lanes["promotion"];
    assert_eq!(promotion.mismatches, 0, "the promotion lane holds parity");
    assert_eq!(
        promotion
            .reuse
            .get(&ReuseClass::Experimental)
            .copied()
            .unwrap_or(0),
        0,
        "the promotion lane admits no Experimental reuse"
    );
    for class in [
        ReuseClass::Exact,
        ReuseClass::Validated,
        ReuseClass::Conservative,
    ] {
        assert!(
            promotion.reuse.get(&class).copied().unwrap_or(0) > 0,
            "the corpus exercises a {class} reuse"
        );
    }
    let interactive = &lanes["interactive"];
    assert!(
        interactive
            .reuse
            .get(&ReuseClass::Experimental)
            .copied()
            .unwrap_or(0)
            > 0,
        "the interactive lane exercises the Experimental heuristic"
    );

    if std::env::var_os("INCREMENTAL_BLESS").is_some() {
        std::fs::write(GOLDEN, &report).expect("write the golden report");
    }
    let golden = std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|e| panic!("{GOLDEN}: {e} (regenerate with INCREMENTAL_BLESS=1)"));
    assert!(
        golden == report,
        "the retained report is stale; regenerate with INCREMENTAL_BLESS=1 and review the diff"
    );
}
