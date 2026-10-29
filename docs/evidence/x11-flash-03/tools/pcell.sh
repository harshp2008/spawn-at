#!/bin/sh
# pcell: one probe cell = one `xflash run` batch of ONE app in the private Shell, then the probe's
# output collected next to it and both analysers run. Thin wrapper: it only calls binaries.
# The private Shell must be running with the probe (PROBE=1 PROBE_CFG='{"pixels":"png"}' psh-start.sh).
#
# Usage: pcell.sh LABEL APP [xflash run options...]
#   e.g. pcell.sh val-xterm xterm --reps 10 --seed 31
#        pcell.sh neg-editor-native gnome-text-editor --reps 0 --ctl-reps 20 --seed 32
# Output: scratch/xflash03/cells/LABEL/ (runs.jsonl, ext.log, probe/, probe-runs.csv, ...).
# `spawn-at` is taken from scratch/xflash03/target-main/debug (built from this tree, src == main).
root=$(cd "$(dirname "$0")/../../../.." && pwd)
t="$root/docs/evidence/x11-flash-03/tools"
label=${1:?usage: pcell.sh LABEL APP [xflash run options...]}
app=${2:?usage: pcell.sh LABEL APP [xflash run options...]}
shift 2
case "$label" in */*|.*) echo "pcell: bad label $label" >&2; exit 2;; esac
out="$root/scratch/xflash03/cells/$label"
[ -e "$out" ] && { echo "pcell: $out exists; choose another label" >&2; exit 2; }
mkdir -p "$out"
PATH="$root/scratch/xflash03/target-main/debug:$PATH"
export PATH
# Empty the probe's directory so this cell's windows are the only ones in it.
sh "$t/probe-collect.sh" "$root/scratch/xflash03/cells/_before_$label" >/dev/null
b="$root/scratch/xflash03/cells/_before_$label"
if findmnt -M "$b" >/dev/null 2>&1; then echo "pcell: $b is a mount point, not removing" >&2; exit 2; fi
rm -rf "$b"
sh "$t/psh-batch.sh" --app "$app" --out "$out" "$@" || { echo "pcell: batch failed (exit $?)" >&2; exit 3; }
sh "$t/probe-collect.sh" "$out"
"$t/target/debug/xflash" analyze --batch "$out" > "$out/analyze.txt" 2>&1
"$t/target/debug/xflash" probe-analyze --probe-dir "$out/probe" --runs "$out/runs.jsonl" \
  --runs-csv "$out/runs.csv" --out "$out"
