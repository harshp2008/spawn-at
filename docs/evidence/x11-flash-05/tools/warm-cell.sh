#!/bin/sh
# warm-cell.sh LABEL APP CLASS "CMD" -- one warm cell: start ONE keeper window of CLASS in the
# private session with CMD (a shell string, run inside `psh`), wait for it, run one pcell with
# XFLASH_KEEP_PIDS=<keeper pid>, then close the keeper window and stop the keeper pid (both
# recorded; nothing is matched by name).
#
# The keeper is started before pcell, which empties the probe directory first, so the keeper's
# probe rows are not part of the cell. `xflash run` ignores the keeper window (it is in the
# batch's baseline) and the keeper pid (XFLASH_KEEP_PIDS, tools/src/run.rs).
set -eu
root=$(cd "$(dirname "$0")/../../../.." && pwd)
t="$root/docs/evidence/x11-flash-03/tools"
label=${1:?usage: warm-cell.sh LABEL APP CLASS CMD}
app=${2:?usage: warm-cell.sh LABEL APP CLASS CMD}
class=${3:?usage: warm-cell.sh LABEL APP CLASS CMD}
cmd=${4:?usage: warm-cell.sh LABEL APP CLASS CMD}
shift 4
[ -n "${SPAWN_AT_BIN_DIR:-}" ] || { echo "warm-cell: SPAWN_AT_BIN_DIR is not set" >&2; exit 2; }
cli="$SPAWN_AT_BIN_DIR/spawn-at"
[ -x "$cli" ] || { echo "warm-cell: $cli is not executable" >&2; exit 2; }
log="$root/scratch/xflash05/logs/keeper-$label.log"
mkdir -p "$root/scratch/xflash05/logs"

# 1. Keeper. The command runs inside the private session and its pid is the recorded child. The
# window's own pid may differ (Chrome's launcher exits and the browser process owns the window),
# so both are recorded and both are stopped at the end; the allowlist gets both.
pid=$(sh "$t/psh" sh -c "$cmd >/dev/null 2>&1 & echo \$!") || { echo "warm-cell: keeper launch failed" >&2; exit 3; }
case "$pid" in ''|*[!0-9]*) echo "warm-cell: keeper pid '$pid' is not a number" >&2; exit 3 ;; esac
echo "keeper $label: launched pid $pid" | tee "$log"

wid=""
wpid=""
i=0
while [ $i -lt 60 ]; do
  entry=$(sh "$t/psh" "$cli" query windows --json 2>/dev/null | jq -c --arg c "$class" '.[] | select(.class == $c or .app_id == $c) | {id, pid, class, app_id, x, y, w, h}' | head -1)
  if [ -n "$entry" ]; then
    wid=$(printf '%s' "$entry" | jq -r '.id')
    wpid=$(printf '%s' "$entry" | jq -r '.pid')
    break
  fi
  sleep 0.25
  i=$((i+1))
done
if [ -z "$wid" ]; then
  echo "warm-cell: keeper window of class $class did not appear; killing launched pid $pid" >&2
  kill -TERM "$pid" 2>/dev/null
  exit 4
fi
keep="$pid"
[ "$wpid" != "$pid" ] && keep="$pid,$wpid"
echo "keeper $label: window $wid window_pid $wpid launched_pid $pid class $class allow=$keep geom=$entry" | tee -a "$log"
sleep 2

# 2. The cell.
set +e
XFLASH_KEEP_PIDS="$keep" sh "$t/pcell.sh" "$label" "$app" "$@"
rc=$?
set -e

# 3. Stop the keeper: graceful close of its window, then only the pids recorded above.
sh "$t/psh" "$cli" close --id "$wid" >/dev/null 2>&1
for p in $pid $wpid; do
  i=0
  while [ $i -lt 20 ]; do
    kill -0 "$p" 2>/dev/null || break
    sleep 0.25
    i=$((i+1))
  done
  if kill -0 "$p" 2>/dev/null; then
    kill -TERM "$p" 2>/dev/null
    i=0
    while [ $i -lt 20 ]; do
      kill -0 "$p" 2>/dev/null || break
      sleep 0.25
      i=$((i+1))
    done
  fi
  if kill -0 "$p" 2>/dev/null; then
    echo "warm-cell: keeper pid $p still alive" >&2
    exit 5
  fi
done
echo "keeper $label: stopped pids $pid $wpid"
exit "$rc"
