#!/bin/sh
# gate.sh on|off: write the sandbox's SPAWN_AT_CONFIG (the file `psh` exports as
# SPAWN_AT_CONFIG=$SB/none.toml). The PR #95 extension keeps its X11 texture gate unless the CLI
# sends `WaitForCommit` with strategy=legacy; `[wait] strategy = "legacy"` makes spawn-at send
# exactly that. `on` writes an empty file, the default. No restart is needed: spawn-at reads the
# file on every invocation and the extension gets the strategy in the instruction.
set -eu
root=$(cd "$(dirname "$0")/../../../.." && pwd)
SB=$(cat "$root/scratch/xflash02/priv/CURRENT")
case "${1:?gate.sh on|off}" in
  on) : > "$SB/none.toml" ;;
  off) printf '[wait]\nstrategy = "legacy"\n' > "$SB/none.toml" ;;
  *) echo "gate.sh: expected on or off" >&2; exit 2 ;;
esac
printf 'gate %s: SPAWN_AT_CONFIG=%s\n' "$1" "$SB/none.toml"
cat "$SB/none.toml"
