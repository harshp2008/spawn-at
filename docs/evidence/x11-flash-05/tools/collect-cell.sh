#!/bin/sh
# collect-cell.sh LABEL [CELLDIR]: copy the measured files of one cell from the private run tree
# (scratch/xflash03/cells/LABEL) into docs/evidence/x11-flash-05/data/LABEL, gzip the probe log,
# and write the binary and extension checksums next to it. CELLDIR overrides the source.
set -eu
root=$(cd "$(dirname "$0")/../../../.." && pwd)
label=${1:?usage: collect-cell.sh LABEL [CELLDIR]}
src=${2:-$root/scratch/xflash03/cells/$label}
dst="$root/docs/evidence/x11-flash-05/data/$label"
[ -d "$src" ] || { echo "collect-cell: $src is missing" >&2; exit 2; }
mkdir -p "$dst"
for f in runs.jsonl runs.csv summary.md analyze.txt probe-runs.csv probe-summary.md meta.json end.json progress.jsonl binary.txt; do
  [ -f "$src/$f" ] && cp "$src/$f" "$dst/$f"
done
# The extension log and the X observer log are large text; keep them gzipped only.
for f in ext.log xobs.jsonl; do
  if [ -f "$src/$f" ]; then gzip -9 -c "$src/$f" > "$dst/$f.gz"; fi
done
if [ -f "$src/probe/probe.jsonl" ]; then
  gzip -9 -c "$src/probe/probe.jsonl" > "$dst/probe.jsonl.gz"
fi
SB=$(cat "$root/scratch/xflash02/priv/CURRENT" 2>/dev/null || true)
if [ -n "$SB" ] && [ -f "$SB/ext-sha256.txt" ]; then
  cp "$SB/ext-sha256.txt" "$dst/ext-sha256.txt"
fi
printf 'copied %s -> %s\n' "$src" "$dst"
du -sh "$dst"
