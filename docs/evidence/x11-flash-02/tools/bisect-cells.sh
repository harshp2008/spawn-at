#!/bin/sh
# bisect-cells: runs the bisect cells, each in a FRESH private headless Shell, never on the live session.
# Usage (repo root): docs/evidence/x11-flash-02/tools/bisect-cells.sh OUTDIR CELL...
# CELL is one of c1..c9 (see the case list). After a private-Shell crash the next cell gets a new Shell.
root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$root" || exit 1
T=docs/evidence/x11-flash-02/tools
out=$1; shift
mkdir -p "$out"
live0=$(pgrep -x gnome-shell | head -1); live0s=$(awk '{print $22}' "/proc/$live0/stat")
for c in "$@"; do
  case $c in
    c1) a="--label c1_xterm_spawnat_obs --app xterm --observer";;
    c2) a="--label c2_editor_wayland_plain --app gnome-text-editor --wayland --plain";;
    c3) a="--label c3_editor_wayland_spawnat --app gnome-text-editor --wayland";;
    c4) a="--label c4_editor_x11_plain --app gnome-text-editor --plain";;
    c5) a="--label c5_editor_x11_spawnat --app gnome-text-editor";;
    c6) a="--label c6_editor_x11_spawnat_nostartupid --app gnome-text-editor --unset-startup";;
    c7) a="--label c7_editor_x11_spawnat_fitid --app gnome-text-editor --startup-id spawn-at-fit_TIME123456789";;
    c8) a="--label c8_editor_x11_spawnat_obs --app gnome-text-editor --observer";;
    c9) a="--label c9_eog_x11_spawnat --app eog";;
    c14) a="--label c14_xterm_plain --app xterm --plain";;
    c15) a="--label c15_editor_x11_spawnat_no_dsid --app gnome-text-editor --unset DESKTOP_STARTUP_ID";;
    c16) a="--label c16_editor_x11_spawnat_no_token --app gnome-text-editor --unset XDG_ACTIVATION_TOKEN";;
    c17) a="--label c17_editor_x11_plain_with_ids --app gnome-text-editor --plain --setenv DESKTOP_STARTUP_ID=spawn-at-1-2_TIME3 --setenv XDG_ACTIVATION_TOKEN=spawn-at-1-2_TIME3";;
    c18) a="--label c18_editor_x11_plain_dsid_only --app gnome-text-editor --plain --setenv DESKTOP_STARTUP_ID=spawn-at-1-2_TIME3";;
    c19) a="--label c19_editor_x11_plain_token_only --app gnome-text-editor --plain --setenv XDG_ACTIVATION_TOKEN=spawn-at-1-2_TIME3";;
    c11) a="--label c11_editor_x11_spawnat_bgwayland --app gnome-text-editor --bg-window";;
    c12) a="--label c12_xterm_spawnat_bgwayland --app xterm --bg-window";;
    c13) a="--label c13_eog_x11_spawnat_bgwayland --app eog --bg-window";;
    c10a) a="--label c10a_editor_x11_wmdelete --app gnome-text-editor --close wmdelete";;
    c10b) a="--label c10b_editor_x11_key --app gnome-text-editor --close key";;
    *) echo "unknown cell $c" >&2; exit 2;;
  esac
  [ "$(pgrep -x gnome-shell | head -1)" = "$live0" ] && [ "$(awk '{print $22}' "/proc/$live0/stat")" = "$live0s" ] || { echo "LIVE SHELL CHANGED before $c: stop" | tee -a "$out/bisect.log"; exit 9; }
  $T/psh-stop.sh "pre-$c" >/dev/null 2>&1
  $T/psh-start.sh >/dev/null 2>&1 || { echo "$c: private session did not start" | tee -a "$out/bisect.log"; exit 8; }
  SB=$(cat scratch/xflash02/priv/CURRENT)
  echo "=== $c $(date +%T) private shell $(cat "$SB/shell.pid")" >> "$out/bisect.log"
  # shellcheck disable=SC2086
  systemd-run --user --scope --quiet -p MemoryMax=3G -p MemorySwapMax=0 -- \
    $T/psh $T/target/debug/xflash cell $a ${XFLAGS:-} --runs ${RUNS:-3} --out "$out" \
    --shell-pid-file "$SB/shell.pid" --shell-log "$SB/log/shell.log" >> "$out/bisect.log" 2>&1
  rc=$?
  echo "=== $c rc=$rc $(date +%T)" >> "$out/bisect.log"
  cp "$SB/log/shell.log" "$out/$c.shell.log" 2>/dev/null
  $T/psh-stop.sh "post-$c" >/dev/null 2>&1
done
echo "ALLDONE" >> "$out/bisect.log"
