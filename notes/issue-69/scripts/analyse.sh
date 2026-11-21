#!/usr/bin/env bash
# Compare run sets of #69. Usage: analyse.sh RUNS_DIR BASE SET...
# RUNS_DIR/<set>/seed-S/{log.txt,boundaries_*.csv,mesh.csv} as written by `runs`.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
notes=$(cd "$here/../.." && pwd)
runs=$(cd "$1" && pwd); base=$2; shift 2
for set in "$base" "$@"; do
  for d in "$runs/$set"/seed-*; do ln -sfn log.txt "$d/log"; done
done
for set in "$@"; do
  echo "## $base vs $set"
  python3 "$notes/issue-43/compare.py" --a "$runs/$base"/seed-* --b "$runs/$set"/seed-*
  mkdir -p "$runs/cmp/$base" "$runs/cmp/$set"
  for s in "$base" "$set"; do
    for d in "$runs/$s"/seed-*; do i=${d##*seed-}; mkdir -p "$runs/cmp/$s/s$i"; ln -sfn "$d/log.txt" "$runs/cmp/$s/s$i/log"; done
  done
  python3 "$notes/realism-batch-4/scripts/compare.py" "$base=$runs/cmp/$base" "$set=$runs/cmp/$set"
done
