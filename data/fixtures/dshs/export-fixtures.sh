#!/usr/bin/env bash
# Re-creates the fixtures in this directory from a snapshot store that holds them.
#
#   data/fixtures/dshs/export-fixtures.sh [STORE_DIR]      (default data/snapshots)
#
# For each `name<TAB>sha256` line of fixtures.tsv it copies the stored blob byte for byte
# (no trimming, no edits) to `name` and writes the store's first retrieval-log line for that
# digest to `name.retrieval.json`. To fill a store first:
#
#   koplik-ingest fetch census-counties
#   koplik-ingest fetch dshs-live
#   koplik-ingest fetch dshs-reports
#   koplik-ingest fetch dshs-wayback            # about 130 captures, one every 8 seconds
#
# The Internet Archive's captures are immutable, so a re-fetch gives the same bytes and the
# same digests; the live DSHS page and PDF (`page-live-*`) are the bytes served on the
# retrieval date and will have changed since. Every file is checked against its digest.
set -euo pipefail
store="${1:-data/snapshots}"
here="$(cd "$(dirname "$0")" && pwd)"
while IFS=$'\t' read -r name sha; do
  [ -n "$name" ] || continue
  blob="$store/blobs/${sha:0:2}/$sha"
  cp "$blob" "$here/$name"
  [ "$(sha256sum "$here/$name" | cut -d' ' -f1)" = "$sha" ] || { echo "digest mismatch: $name" >&2; exit 1; }
  grep -m1 "\"sha256\":\"$sha\"" "$store/retrievals.jsonl" > "$here/$name.retrieval.json"
done < "$here/fixtures.tsv"
