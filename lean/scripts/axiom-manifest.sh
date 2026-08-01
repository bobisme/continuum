#!/bin/sh
# Regenerate (default) or verify (--check) the PR-4A T0/T1 axiom manifest.
#
#   sh scripts/axiom-manifest.sh            # rewrite artifacts/axiom-manifest-t0-t1.{json,txt}
#   sh scripts/axiom-manifest.sh --check    # fail if the checked-in artifacts are stale
#
# The manifest is required by ADR-0035 and covers the RFC 0012 T0/T1 ladder.
set -eu

cd "$(dirname "$0")/.."

json=artifacts/axiom-manifest-t0-t1.json
txt=artifacts/axiom-manifest-t0-t1.txt
mode=${1:-}

lake build

if [ "$mode" = "--check" ]; then
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  if [ ! -f "$json" ] || [ ! -f "$txt" ]; then
    echo "axiom manifest is missing: run sh scripts/axiom-manifest.sh" >&2
    exit 1
  fi
  cp "$json" "$tmp/manifest.json"
  cp "$txt" "$tmp/manifest.txt"
  lake env lean AxiomManifest.lean >/dev/null
  status=0
  diff -u "$tmp/manifest.json" "$json" || status=1
  diff -u "$tmp/manifest.txt" "$txt" || status=1
  if [ "$status" -ne 0 ]; then
    cp "$tmp/manifest.json" "$json"
    cp "$tmp/manifest.txt" "$txt"
    echo "axiom manifest is stale: regenerate with sh scripts/axiom-manifest.sh" >&2
    exit 1
  fi
  echo "axiom manifest is up to date"
  exit 0
fi

lake env lean AxiomManifest.lean
