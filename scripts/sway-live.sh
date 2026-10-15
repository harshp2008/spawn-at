#!/usr/bin/env bash
# scripts/sway-live.sh
# Live tests of the EXPERIMENTAL Sway backend, in a private headless Sway that
# scripts/sway-headless.sh starts and stops. It never touches your desktop session.
#
# Usage: scripts/sway-live.sh [--dry-run]
#   --dry-run  print what would be started, spawned and closed; start and spawn nothing.
#
# Needs: sway, swaymsg, jq. Builds target/debug/spawn-at and the fixture window if missing.
#
# What it does (two private sessions, one after the other):
#  A. one 1920x1080 output: the anchors, margins and sizes of scripts/smoke-live.sh with the repo's
#     fixture window (center; top-right with margin 24; bottom-right with margin 16 and a size
#     below the minimum; bottom-right with a normal size; top-left with --no-wait), then
#     transform, close, query windows and query layout, each compared with what `swaymsg` says;
#     and the forced-Sway error cases (SWAYSOCK unset, empty, missing).
#  B. a scale-2 output with a 30 px bar plus a second output: query layout against swaymsg, and
#     placement on monitor 0 and on monitor 1.
# Every position is checked within 1 px against the work area that swaymsg reports.
#
# Safety (a window or process is only touched if this script created it):
#  - sway runs only through scripts/sway-headless.sh (own runtime dir, own sockets, headless, no
#    XWayland); every spawn-at call gets SWAYSOCK and WAYLAND_DISPLAY of that private session
#    and no DISPLAY, and the script refuses to run a call if SWAYSOCK is not inside the private
#    directory;
#  - windows are closed by the id recorded from the private session (`spawn-at close --id`);
#  - a fixture process is killed only if it is still alive after that, its name is `fixture`
#    and its environment names the private Wayland socket;
#  - sway is stopped with `sway-headless.sh stop`, which kills only the PID it recorded;
#  - if gnome-shell is running, its PID must be unchanged at the end.

set -uo pipefail

DRY_RUN=0
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=1 ;;
        -h|--help) sed -n 2,31p "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "unknown argument: $arg (try --dry-run)" >&2; exit 2 ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
HARNESS="$SCRIPT_DIR/sway-headless.sh"
BIN="$PROJECT_ROOT/target/debug/spawn-at"
# Relative to PROJECT_ROOT so the path has no spaces (the command is word-split below).
FIXTURE="target/debug/fixture"
cd "$PROJECT_ROOT"

echo "========================================================"
echo "   spawn-at Sway live tests (private headless Sway)     "
echo "========================================================"

if [ "$DRY_RUN" -eq 1 ]; then
    echo "DRY RUN: nothing is started, spawned or closed."
    echo
    echo "Would start two private sessions with scripts/sway-headless.sh:"
    "$HARNESS" start --dry-run
    echo
    echo "Session A (1920x1080): spawn the fixture window with"
    echo "  --anchor center | --anchor top-right -m 24 | --anchor bottom-right -m 16 --size 30 20"
    echo "  | --anchor bottom-right -m 16 --size 600 400 | --anchor top-left -m 10 --size 500 300 --no-wait"
    echo "  then transform --id, close --id, close --class, query windows, query layout,"
    echo "  and SPAWN_AT_BACKEND=sway with SWAYSOCK unset, empty and missing (expect exit 1)."
    echo "Session B (scale 2, 30 px bar, second output): query layout; spawn on monitor 0 and 1."
    echo "Each spawn: SPAWN_AT_BACKEND=sway SWAYSOCK=<private> WAYLAND_DISPLAY=<private>, DISPLAY unset."
    echo "Each window is closed by its recorded id; sway is stopped with sway-headless.sh stop."
    exit 0
fi

for tool in sway swaymsg jq; do
    command -v "$tool" >/dev/null || { echo "sway-live: '$tool' is not installed" >&2; exit 2; }
done

if [ ! -f "$BIN" ] || [ ! -f "$PROJECT_ROOT/$FIXTURE" ]; then
    echo "==> Building spawn-at and the fixture window..."
    cargo build --manifest-path "$PROJECT_ROOT/Cargo.toml" -p spawn-at -p spawn-at-verify --features spawn-at-verify/fixture || exit 2
fi

GNOME_BEFORE="$(pgrep -x gnome-shell 2>/dev/null | tr '\n' ' ')"
DIR=""
TMPCONF=""
PASSES=0
FAILS=0

pass() { PASSES=$((PASSES + 1)); echo "  PASS: $*"; }
fail() { FAILS=$((FAILS + 1)); echo "  FAIL: $*"; }

# --- the private session ----------------------------------------------------------------------

# Kills fixture processes of the private session that are still alive: only processes named
# `fixture` whose environment holds the private Wayland socket path.
reap_fixtures() {
    [ -n "$DIR" ] || return 0
    local env_file pid
    for env_file in /proc/[0-9]*/environ; do
        pid="${env_file#/proc/}"; pid="${pid%/environ}"
        [ "$(cat "/proc/$pid/comm" 2>/dev/null)" = "fixture" ] || continue
        if tr '\0' '\n' < "$env_file" 2>/dev/null | grep -q "^WAYLAND_DISPLAY=$DIR/"; then
            kill "$pid" 2>/dev/null
        fi
    done
}

stop_session() {
    if [ -n "$DIR" ]; then
        reap_fixtures
        "$HARNESS" stop --dir "$DIR" || fail "could not stop the private sway in $DIR"
        DIR=""
        # no stale pointer to the removed session in this script's environment
        unset SPAWN_AT_SWAY_DIR SWAYSOCK WAYLAND_DISPLAY WLR_BACKENDS
    fi
}

cleanup() { stop_session; [ -z "$TMPCONF" ] || rm -f "$TMPCONF"; }
trap cleanup EXIT
trap 'exit 130' INT TERM

start_session() { # [extra-config-file]
    local out
    if [ -n "${1:-}" ]; then out="$("$HARNESS" start --extra-config "$1")" || return 1
    else out="$("$HARNESS" start)" || return 1; fi
    eval "$out"
    DIR="$SPAWN_AT_SWAY_DIR"
    case "$SWAYSOCK" in "$DIR"/*) ;; *) echo "sway-live: SWAYSOCK is outside the private dir; stopping" >&2; stop_session; return 1 ;; esac
    echo "==> private sway in $DIR"
}

# spawn-at against the private session only.
sa() {
    case "$SWAYSOCK" in "$DIR"/*) ;; *) echo "sway-live: refusing to run outside the private session" >&2; return 99 ;; esac
    env -u DISPLAY SPAWN_AT_BACKEND=sway SWAYSOCK="$SWAYSOCK" WAYLAND_DISPLAY="$WAYLAND_DISPLAY" "$BIN" "$@"
}

sm() { swaymsg -s "$SWAYSOCK" "$@"; }

# All client windows of the private sway, one JSON object per line.
tree_windows() {
    sm -t get_tree -r | jq -c '.. | objects | select(.pid? != null and (.type=="con" or .type=="floating_con")) | {id,pid,name,app_id,rect}'
}

window_by_name() { tree_windows | jq -c --arg n "$1" 'select(.name == $n)' | head -1; }

# The work area (usable rectangle) of output number $1 (0-based, in the order sway lists them).
workarea_of_output() {
    local name
    name="$(sm -t get_outputs -r | jq -r --argjson i "$1" '[.[] | select(.active)][$i].name')"
    sm -t get_workspaces -r | jq -c --arg o "$name" '[.[] | select(.output == $o and .visible)][0].rect'
}

# Expected top-left of a w x h window for an anchor and margin in work area "x y W H".
expected_pos() { # anchor margin wx wy ww wh w h
    local a=$1 m=$2 wx=$3 wy=$4 ww=$5 wh=$6 w=$7 h=$8
    case "$a" in
        center)       echo "$((wx + (ww - w) / 2)) $((wy + (wh - h) / 2))" ;;
        top-left)     echo "$((wx + m)) $((wy + m))" ;;
        top-right)    echo "$((wx + ww - m - w)) $((wy + m))" ;;
        bottom-left)  echo "$((wx + m)) $((wy + wh - m - h))" ;;
        bottom-right) echo "$((wx + ww - m - w)) $((wy + wh - m - h))" ;;
    esac
}

abs() { local v=$1; [ "$v" -lt 0 ] && v=$((-v)); echo "$v"; }

# Waits up to 3 s for a window with this name to appear in the private sway; prints its JSON.
wait_window() {
    local i w
    for i in $(seq 1 60); do
        w="$(window_by_name "$1")"
        [ -n "$w" ] && { echo "$w"; return 0; }
        sleep 0.05
    done
    return 1
}

# Closes window id $1 (pid $2) the way a user would: `spawn-at close --id`, then checks it is gone.
close_and_check() { # id pid label
    sa close --id "$1" --no-wait >/dev/null 2>&1
    local i
    for i in $(seq 1 60); do
        [ -z "$(tree_windows | jq -c --argjson id "$1" 'select(.id == $id)')" ] && break
        sleep 0.05
    done
    if [ -z "$(tree_windows | jq -c --argjson id "$1" 'select(.id == $id)')" ]; then pass "$3: close removed the window"
    else fail "$3: the window is still in the tree after close"; fi
    for i in $(seq 1 40); do kill -0 "$2" 2>/dev/null || break; sleep 0.05; done
    if kill -0 "$2" 2>/dev/null; then
        fail "$3: the fixture process $2 is still alive after close"
        reap_fixtures
    else pass "$3: the fixture process exited"; fi
}

# spawn-at spawn --json with the fixture; checks the position within 1 px and the size.
# Usage: spawn_case LABEL TITLE MONITOR ANCHOR MARGIN [SIZE_W SIZE_H] [-- extra spawn flags]
spawn_case() {
    local label=$1 title=$2 mon=$3 anchor=$4 margin=$5 sw="" sh="" extra=()
    shift 5
    if [ "${1:-}" != "" ] && [ "${1:-}" != "--" ]; then sw=$1; sh=$2; shift 2; fi
    [ "${1:-}" = "--" ] && shift
    extra=("$@")
    echo "--- $label"
    local flags=(--anchor "$anchor")
    [ "$margin" -gt 0 ] && flags+=(-m "$margin")
    [ -n "$sw" ] && flags+=(--size "$sw" "$sh")
    [ "$mon" != "-" ] && flags+=(--monitor "$mon")
    local out rc
    out="$(sa spawn --json "${flags[@]}" "${extra[@]}" -- $FIXTURE --protocol wayland --title "$title" --exit-after-ms 90000 2>"$DIR/spawn.err")"; rc=$?
    [ "$rc" -eq 0 ] && pass "$label: spawn-at exited 0" || { fail "$label: spawn-at exited $rc: $(cat "$DIR/spawn.err")"; return; }
    grep -q 'Timed out\|post-spawn hook failed' "$DIR/spawn.err" && fail "$label: warning on stderr: $(cat "$DIR/spawn.err")"
    local win
    win="$(wait_window "$title")" || { fail "$label: no window named $title in the private sway"; return; }
    local id pid x y w h
    id="$(jq -r .id <<<"$win")"; pid="$(jq -r .pid <<<"$win")"
    x="$(jq -r .rect.x <<<"$win")"; y="$(jq -r .rect.y <<<"$win")"; w="$(jq -r .rect.width <<<"$win")"; h="$(jq -r .rect.height <<<"$win")"
    local idx=0; [ "$mon" != "-" ] && idx=$mon
    local wa wx wy ww wh
    wa="$(workarea_of_output "$idx")"
    wx="$(jq -r .x <<<"$wa")"; wy="$(jq -r .y <<<"$wa")"; ww="$(jq -r .width <<<"$wa")"; wh="$(jq -r .height <<<"$wa")"
    read -r ex ey <<<"$(expected_pos "$anchor" "$margin" "$wx" "$wy" "$ww" "$wh" "$w" "$h")"
    local dx dy
    dx="$(abs $((x - ex)))"; dy="$(abs $((y - ey)))"
    echo "  window id=$id pid=$pid at ($x,$y) ${w}x${h}; expected ($ex,$ey) in work area ${ww}x${wh}@${wx},${wy}"
    if [ "$dx" -le 1 ] && [ "$dy" -le 1 ]; then pass "$label: position within 1 px (dx=$dx dy=$dy)"
    else fail "$label: position ($x,$y) differs from ($ex,$ey) by ($dx,$dy)"; fi
    if [ -n "$sw" ]; then
        # A client has a minimum size (75x50 for a floating window here): the size may be raised,
        # never lowered, and the position is for the real size.
        if [ "$w" -ge "$sw" ] && [ "$h" -ge "$sh" ]; then pass "$label: size ${w}x${h} is at least the requested ${sw}x${sh}"
        else fail "$label: size ${w}x${h} is below the requested ${sw}x${sh}"; fi
        if [ "$sw" -ge 100 ] && { [ "$w" -ne "$sw" ] || [ "$h" -ne "$sh" ]; }; then fail "$label: size ${w}x${h} is not exactly ${sw}x${sh}"; fi
    fi
    # what spawn-at says it did must be what sway has
    local said
    said="$(jq -r .window_id <<<"$out")"
    if [ "$said" = "$id" ]; then pass "$label: spawn --json window_id $said is the container id"
    else fail "$label: spawn --json window_id '$said', swaymsg id '$id'"; fi
    local rep
    rep="$(sa query windows --json 2>/dev/null | jq -c --argjson id "$id" '.[] | select(.id == $id) | [.x,.y,.w,.h]')"
    if [ "$rep" = "[$x,$y,$w,$h]" ]; then pass "$label: query windows reports $rep as swaymsg does"
    else fail "$label: query windows says '$rep', swaymsg says [$x,$y,$w,$h]"; fi
    LAST_ID=$id; LAST_PID=$pid
    close_and_check "$id" "$pid" "$label"
}

layout_matches_swaymsg() { # label
    local ours theirs
    ours="$(sa query layout --json 2>/dev/null | jq -c '[.monitors[] | [.screen.x,.screen.y,.screen.w,.screen.h,.workarea.x,.workarea.y,.workarea.w,.workarea.h]]')"
    theirs="$(jq -n -c --argjson o "$(sm -t get_outputs -r)" --argjson w "$(sm -t get_workspaces -r)" \
        '[$o[] | select(.active) | . as $out | [$out.rect.x,$out.rect.y,$out.rect.width,$out.rect.height] + ([$w[] | select(.output == $out.name and .visible) | .rect | [.x,.y,.width,.height]][0])]')"
    echo "  spawn-at: $ours"
    echo "  swaymsg:  $theirs"
    if [ -n "$ours" ] && [ "$ours" = "$theirs" ]; then pass "$1: query layout equals swaymsg"; else fail "$1: query layout differs from swaymsg"; fi
}

# --- session A ---------------------------------------------------------------------------------

echo "######## Session A: one 1920x1080 output"
if start_session; then
    layout_matches_swaymsg "A layout"
    spawn_case "A1 center" sway-live-a1 - center 0
    spawn_case "A2 top-right m24" sway-live-a2 - top-right 24
    spawn_case "A3 bottom-right m16 size 30x20 (below the minimum)" sway-live-a3 - bottom-right 16 30 20
    spawn_case "A4 bottom-right m16 size 600x400" sway-live-a4 - bottom-right 16 600 400
    echo "--- A5 top-left m10 size 500x300 --no-wait"
    sa spawn --anchor top-left -m 10 --size 500 300 --no-wait -- $FIXTURE --protocol wayland --title sway-live-a5 --exit-after-ms 90000 >/dev/null 2>&1
    if win="$(wait_window sway-live-a5)"; then
        id="$(jq -r .id <<<"$win")"; pid="$(jq -r .pid <<<"$win")"
        # --no-wait returns before the window may be placed, so allow the placement to land
        for _ in $(seq 1 40); do
            r="$(window_by_name sway-live-a5 | jq -c '[.rect.x,.rect.y,.rect.width,.rect.height]')"
            [ "$r" = "[10,10,500,300]" ] && break; sleep 0.05
        done
        [ "$r" = "[10,10,500,300]" ] && pass "A5: --no-wait placed the window at [10,10,500,300]" || fail "A5: --no-wait left the window at $r"
        close_and_check "$id" "$pid" "A5"
    else fail "A5: no window named sway-live-a5"; fi

    echo "--- A6 transform"
    sa spawn --anchor top-left -m 40 --size 300 200 -- $FIXTURE --protocol wayland --title sway-live-a6 --exit-after-ms 90000 >/dev/null 2>&1
    if win="$(wait_window sway-live-a6)"; then
        id="$(jq -r .id <<<"$win")"; pid="$(jq -r .pid <<<"$win")"
        sa transform --id "$id" --anchor bottom-right -m 25 --size 700 500 >/dev/null 2>&1
        sleep 0.3
        r="$(window_by_name sway-live-a6 | jq -c '[.rect.x,.rect.y,.rect.width,.rect.height]')"
        [ "$r" = "[1195,555,700,500]" ] && pass "A6: transform moved and resized to [1195,555,700,500]" || fail "A6: after transform the window is at $r, expected [1195,555,700,500]"
        close_and_check "$id" "$pid" "A6"
    else fail "A6: no window named sway-live-a6"; fi

    echo "--- A7 close by class"
    sa spawn --anchor center -- $FIXTURE --protocol wayland --app-id dev.spawnat.swaylive7 --title sway-live-a7 --exit-after-ms 90000 >/dev/null 2>&1
    if win="$(wait_window sway-live-a7)"; then
        pid="$(jq -r .pid <<<"$win")"
        sa close --class dev.spawnat.swaylive7 --no-wait >/dev/null 2>&1
        for _ in $(seq 1 60); do [ -z "$(window_by_name sway-live-a7)" ] && break; sleep 0.05; done
        [ -z "$(window_by_name sway-live-a7)" ] && pass "A7: close --class removed the window" || fail "A7: window still present after close --class"
        for _ in $(seq 1 40); do kill -0 "$pid" 2>/dev/null || break; sleep 0.05; done
        kill -0 "$pid" 2>/dev/null && { fail "A7: the fixture process is still alive"; reap_fixtures; } || pass "A7: the fixture process exited"
    else fail "A7: no window named sway-live-a7"; fi

    echo "--- A8 forced Sway without a usable SWAYSOCK is an error, not X11"
    for variant in unset empty missing; do
        case "$variant" in
            unset)   msg="$(env -u DISPLAY -u SWAYSOCK SPAWN_AT_BACKEND=sway "$BIN" query layout 2>&1)"; rc=$? ;;
            empty)   msg="$(env -u DISPLAY SWAYSOCK= SPAWN_AT_BACKEND=sway "$BIN" query layout 2>&1)"; rc=$? ;;
            missing) msg="$(env -u DISPLAY SWAYSOCK="$DIR/no-such.sock" SPAWN_AT_BACKEND=sway "$BIN" query layout 2>&1)"; rc=$? ;;
        esac
        if [ "$rc" -eq 1 ] && grep -q '\$SWAYSOCK' <<<"$msg" && ! grep -q 'X11' <<<"$msg"; then pass "A8 $variant: exit 1, message names \$SWAYSOCK"
        else fail "A8 $variant: rc=$rc msg=$msg"; fi
    done
    left="$(tree_windows)"
    [ -z "$left" ] && pass "A: no window left in the private sway" || fail "A: windows left: $left"
    stop_session
else
    fail "session A did not start"
fi

# --- session B ---------------------------------------------------------------------------------

echo "######## Session B: scale 2, 30 px bar, second output"
TMPCONF="$(mktemp)"
cat > "$TMPCONF" <<'EOF'
output HEADLESS-1 scale 2
bar {
    position top
    status_command while :; do echo x; sleep 60; done
    height 30
}
EOF
if start_session "$TMPCONF"; then
    sm create_output >/dev/null 2>&1
    sleep 0.4
    sm output HEADLESS-2 resolution 1280x720 position 960 0 scale 1 >/dev/null 2>&1
    sleep 0.3
    layout_matches_swaymsg "B layout"
    spawn_case "B1 center on monitor 0 (scale 2, bar)" sway-live-b1 0 center 0
    spawn_case "B2 bottom-right m16 size 400x300 on monitor 1" sway-live-b2 1 bottom-right 16 400 300
    spawn_case "B3 top-left m20 on monitor 1" sway-live-b3 1 top-left 20
    left="$(tree_windows)"
    [ -z "$left" ] && pass "B: no window left in the private sway" || fail "B: windows left: $left"
    stop_session
else
    fail "session B did not start"
fi

# --- end ---------------------------------------------------------------------------------------

strays="$(for e in /proc/[0-9]*/environ; do
    [ -r "$e" ] || continue   # other users' processes cannot be ours
    { tr '\0' '\n' < "$e" | grep -q '^WAYLAND_DISPLAY=/tmp/spawn-at-sway\.' && echo "${e%/environ}"; } 2>/dev/null
done)"
[ -z "$strays" ] && pass "no process of the private sessions is left" || fail "left over: $strays"
GNOME_AFTER="$(pgrep -x gnome-shell 2>/dev/null | tr '\n' ' ')"
[ "$GNOME_BEFORE" = "$GNOME_AFTER" ] && pass "gnome-shell PIDs unchanged ('${GNOME_BEFORE:-none running}')" || fail "gnome-shell PIDs changed: '$GNOME_BEFORE' -> '$GNOME_AFTER'"

echo "========================================================"
echo " passed: $PASSES   failed: $FAILS"
echo "========================================================"
[ "$FAILS" -eq 0 ]
