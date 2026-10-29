#!/bin/sh
# probe-collect: move the probe's output of the private Shell (probe.jsonl and px/) into DEST/probe,
# leaving the sandbox's probe directory empty for the next batch.
# Usage: probe-collect.sh DEST          (DEST is the batch directory, e.g. scratch/xflash03/b_xterm)
# Run it between batches, never during one. It only touches the private sandbox recorded in
# scratch/xflash02/priv/CURRENT and DEST.
root=$(cd "$(dirname "$0")/../../../.." && pwd)
SB=$(cat "$root/scratch/xflash02/priv/CURRENT") || exit 97
dest=${1:?usage: probe-collect.sh DEST}
case "$SB" in /tmp/xflash-priv.*) ;; *) echo "probe-collect: unexpected sandbox $SB" >&2; exit 1;; esac
mkdir -p "$dest/probe"
[ -f "$SB/probe/probe.jsonl" ] && mv "$SB/probe/probe.jsonl" "$dest/probe/probe.jsonl"
if [ -d "$SB/probe/px" ]; then
  mkdir -p "$dest/probe/px"
  # PNG writes are asynchronous: give the last ones a moment to land
  sleep 1
  find "$SB/probe/px" -maxdepth 1 -name '*.png' -exec mv {} "$dest/probe/px/" \;
fi
echo "collected: $(wc -l < "$dest/probe/probe.jsonl" 2>/dev/null) lines, $(ls "$dest/probe/px" 2>/dev/null | wc -l) png in $dest/probe"
