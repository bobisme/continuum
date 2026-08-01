#!/bin/sh
# Regenerate (default) or verify (--check) the PR-4A T0/T1 axiom manifest.
#
#   sh scripts/axiom-manifest.sh            # rewrite artifacts/axiom-manifest-t0-t1.{json,txt}
#   sh scripts/axiom-manifest.sh --check    # fail if the checked-in artifacts are stale
#
# The manifest is required by ADR-0035 and covers the RFC 0012 T0/T1 ladder.
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
  echo "axiom manifest is up to date"
  exit 0
fi

lake env lean AxiomManifest.lean
