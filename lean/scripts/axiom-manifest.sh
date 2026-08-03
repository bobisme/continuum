#!/bin/sh
# Regenerate (default) or verify (--check) the PR-4A T0/T1 axiom manifest.
#
#   sh scripts/axiom-manifest.sh            # rewrite artifacts/axiom-manifest-t0-t1.{json,txt}
#   sh scripts/axiom-manifest.sh --check    # fail if the checked-in artifacts are stale
#                                           # OR if any listed theorem depends on an axiom
#
# The manifest is required by ADR-0035 and covers the RFC 0012 T0/T1 ladder.
#
# Two obligations, two checks, and they are not the same obligation (bn-bou09)
# -----------------------------------------------------------------------------
#
# ADR-0035's decision is that every strongest-assurance result carries "machine-captured
# axiom output", and its consequence is that "undocumented axioms fail release gates".
# That is a *documentation* obligation, and the staleness diff below discharges it
# completely: the manifest is regenerated with `Lean.collectAxioms` — the same source
# `#print axioms` reads — into a scratch directory and compared against the checked-in
# artifacts, so an axiom cannot become undocumented without the gate going red.
#
# It does NOT discharge the second obligation, which is stated elsewhere and is stronger:
#
#   T0/T1 compile with no `sorry` and empty axiom manifests (RFC 0012 theorem ladder;
#   axiom manifests per ADR-0035).
#     — notes/plan/notes/START_HERE_IMPLEMENTATION.md, PR 4a "Exit"
#
#   Lean environment pinned and seed modules kernel-checked; T0/T1 theorems … compile
#   with no `sorry` and empty axiom manifests, per the RFC 0012 theorem ladder and
#   ADR-0035 axiom manifests
#     — notes/plan/plan.md §21, Phase A "Deliver"
#
# "Empty" is a fact about the manifest's contents, and the diff cannot see it: a `sorry`
# landed *together with* a regenerated manifest documents its own `sorryAx` and passes
# the diff, which is exactly how the gate could have been green while the exit line was
# false. The three assertions below close that, and they are deliberately scoped to this
# manifest — the T0/T1 ladder, whose exit line says "empty". They are not a project-wide
# ban on axioms: RFC 0012 §"Permitted foundations are documented" allows documented
# foundations, so a later rung that legitimately used `Classical.choice` would carry its
# own manifest and its own bar. Widening or narrowing this one is a deliberate edit here.
#
# Generation stays total on purpose: an offending axiom must still be *writable* into
# the manifest, so that a developer can see which theorem acquired it. The gate, not the
# generator, is what refuses.
#
# `--check` never writes under `artifacts/` (bn-1ee03). The generator always
# writes `artifacts/…` relative to its working directory, so the check runs it
# with a scratch directory as the working directory and diffs the result against
# the checked-in files; the tree is only ever read. The previous version
# regenerated in place and copied the originals back afterwards, which churned
# the checked-in artifacts on every run and left them overwritten if the check
# was interrupted between the two steps — a gate must not edit what it judges.
set -eu

cd "$(dirname "$0")/.."
root=$(pwd)

json=artifacts/axiom-manifest-t0-t1.json
txt=artifacts/axiom-manifest-t0-t1.txt
mode=${1:-}

lake build

if [ "$mode" = "--check" ]; then
  if [ ! -f "$json" ] || [ ! -f "$txt" ]; then
    echo "axiom manifest is missing: run sh scripts/axiom-manifest.sh" >&2
    exit 1
  fi
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  # The inner shell expands the two variables itself, so paths containing shell
  # metacharacters survive; `lake env` supplies LEAN_PATH and the toolchain.
  MANIFEST_OUT_DIR="$tmp" MANIFEST_SRC="$root/AxiomManifest.lean" \
    lake env sh -c 'cd "$MANIFEST_OUT_DIR" && exec lean "$MANIFEST_SRC"' >/dev/null
  status=0
  diff -u "$json" "$tmp/$json" || status=1
  diff -u "$txt" "$tmp/$txt" || status=1
  if [ "$status" -ne 0 ]; then
    echo "axiom manifest is stale: regenerate with sh scripts/axiom-manifest.sh" >&2
    exit 1
  fi

  # The manifest is current. Now assert it is *empty*, per the PR-4a / Phase A exit
  # line quoted in this file's header. Three independent readings of the same fact, so
  # that no single formatting change can make the check pass vacuously:
  #
  #   1. the generator's own count field;
  #   2. every row's axiom list in the JSON, read without trusting that field;
  #   3. the human transcript, which spells a non-empty row with different words
  #      ("depends on axioms: [...]" rather than "does not depend on any axioms").
  #
  # Pure text over the checked-in artifacts the diff has just validated, so this half
  # needs no toolchain and cannot rewrite what it judges.
  empty=0
  grep -q '^  "theoremsWithAxioms": 0,$' "$json" || {
    echo "axiom manifest: theoremsWithAxioms is not 0" >&2
    empty=1
  }
  if grep -n '"axioms": \[[^]]' "$json" >&2; then
    echo "axiom manifest: the theorem rows above depend on axioms" >&2
    empty=1
  fi
  if grep -n 'depends on axioms:' "$txt" >&2; then
    echo "axiom manifest: the transcript rows above depend on axioms" >&2
    empty=1
  fi
  if [ "$empty" -ne 0 ]; then
    echo "T0/T1 must compile with no \`sorry\` and empty axiom manifests" >&2
    echo "  — notes/plan/notes/START_HERE_IMPLEMENTATION.md, PR 4a Exit; plan.md §21" >&2
    exit 1
  fi

  echo "axiom manifest is up to date, and every T0/T1 theorem is axiom-free"
  exit 0
fi

lake env lean AxiomManifest.lean
