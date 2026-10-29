#!/bin/sh
# psh-batch: one xflash batch inside the PRIVATE session, in a 3 GB memory scope.
# systemd-run runs with the live environment (it only creates the scope); `psh` then switches to the
# private environment and refuses unless the private session is up.
# Usage: psh-batch.sh --app xterm --out DIR [more xflash run options]
root=$(cd "$(dirname "$0")/../../../.." && pwd)
SB=$(cat "$root/scratch/xflash02/priv/CURRENT") || exit 97
exec systemd-run --user --scope --quiet -p MemoryMax=3G -p MemorySwapMax=0 -- \
  "$root/docs/evidence/x11-flash-03/tools/psh" \
  "$root/docs/evidence/x11-flash-03/tools/target/debug/xflash" run \
  --shell-pid-file "$SB/shell.pid" --shell-log "$SB/log/shell.log" --state "$SB/state.json" "$@"
