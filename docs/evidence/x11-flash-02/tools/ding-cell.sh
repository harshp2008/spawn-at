#!/bin/sh
# ding-cell: run ONE xflash cell in a FRESH DING-only private headless Shell (X11-FLASH-02i).
# "DING-only" = the spawn-at extension plus Desktop Icons NG (ding@rastersoft.com) and nothing else,
# built by psh-start.sh with H7=1 and every other live extension in H7_SKIP.
# Usage: ding-cell.sh LABEL OUTDIR --app APP [xflash cell options...]
# Before and after the cell the live gnome-shell pid and start time are compared; if either changed
# the script stops with exit 9. It never signals a process it did not record (psh-stop.sh does that).
# Extra Shell environment (private only): DING_ENV='export MUTTER_DEBUG=focus,startup,ping'.
# The binary under test is $SPAWN_AT when set (a throwaway-worktree build), else the repo's own
# target/debug/spawn-at; it is recorded with its sha256 in the runner log.
set -u
root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$root" || exit 1
T=docs/evidence/x11-flash-02/tools
h=docs/evidence/x11-flash-02/data/h7
: "${SPAWN_AT:=$root/target/debug/spawn-at}"
export SPAWN_AT
label=$1; shift
out=$1; shift
[ -n "$label" ] && [ -n "$out" ] || { echo "usage: ding-cell.sh LABEL OUTDIR --app APP ..." >&2; exit 2; }
mkdir -p "$out"
live0=$(pgrep -x gnome-shell | head -1)
live0s=$(awk '{print $22}' "/proc/$live0/stat" 2>/dev/null)
check_live() {
  p=$(pgrep -x gnome-shell | head -1)
  s=$(awk '{print $22}' "/proc/$p/stat" 2>/dev/null)
  [ "$p" = "$live0" ] && [ "$s" = "$live0s" ]
}
check_live || { echo "LIVE SHELL CHANGED before $label: stop" | tee -a "$out/ding-cells.log"; exit 9; }
skip=""
for u in $(cat "$h/user-extensions.txt" "$h/system-extensions.txt"); do
  [ "$u" = "ding@rastersoft.com" ] && continue
  skip="$skip $u"
done
# DING_ON=0 starts the plain psh-start.sh set (spawn-at's extension only), for the no-DING control.
if [ "${DING_ON:-1}" = 0 ]; then
  PSH_SHELL_ENV="${DING_ENV:-}" $T/psh-start.sh >/dev/null 2>&1 || { echo "$label: private session did not start" | tee -a "$out/ding-cells.log"; exit 8; }
else
  H7=1 H7_SKIP="$skip" PSH_SHELL_ENV="${DING_ENV:-}" \
    $T/psh-start.sh >/dev/null 2>&1 || { echo "$label: private session did not start" | tee -a "$out/ding-cells.log"; exit 8; }
fi
SB=$(cat scratch/xflash02/priv/CURRENT)
echo "=== $label $(date +%T) private shell $(cat "$SB/shell.pid") env=${DING_ENV:-none} spawn-at=$SPAWN_AT ding=${DING_ON:-1}" >> "$out/ding-cells.log"
sha256sum "$SPAWN_AT" >> "$out/ding-cells.log" 2>&1
# Confirm the set really is DING-only, and record the private window list before the first run.
$T/psh sh -c 'gsettings get org.gnome.shell enabled-extensions' >> "$out/ding-cells.log" 2>&1
# The extension exports its D-Bus object a moment after the Shell's socket appears; a cell started
# too early sees "protocol 0" and every spawn-at call fails (one crash was spent on that on the
# first attempt). Wait until `query windows --json` answers with a JSON array.
i=0
while [ $i -lt 40 ]; do
  q=$($T/psh "$SPAWN_AT" query windows --json 2>/dev/null)
  case "$q" in \[*) break;; esac
  sleep 0.5; i=$((i+1))
done
printf '%s\n' "$q" >> "$out/ding-cells.log"
# A short extra settle: one cell (g2) lost its first run because `spawn-at spawn` still saw the
# extension mid-enable after `query windows` already answered.
sleep 2
systemd-run --user --scope --quiet -p MemoryMax=3G -p MemorySwapMax=0 -- \
  $T/psh $T/target/debug/xflash cell --label "$label" --out "$out" \
  --shell-pid-file "$SB/shell.pid" --shell-log "$SB/log/shell.log" "$@" >> "$out/ding-cells.log" 2>&1
rc=$?
echo "=== $label rc=$rc $(date +%T)" >> "$out/ding-cells.log"
cp "$SB/log/shell.log" "$out/$label.shell.log" 2>/dev/null
$T/psh-stop.sh "post-$label" >/dev/null 2>&1
check_live || { echo "LIVE SHELL CHANGED after $label: stop" | tee -a "$out/ding-cells.log"; exit 9; }
exit $rc
