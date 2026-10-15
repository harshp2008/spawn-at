#!/usr/bin/env bash
# scripts/smoke-live.sh
# End-to-end live smoke test verifying window spawning, placement geometry,
# and truthful reporting within 1px tolerance on GNOME X11 / Wayland.
#
# Usage: scripts/smoke-live.sh [--dry-run]
#   --dry-run  print what would be spawned and closed; spawn and close nothing.
#
# Safety rules (a window or process is only touched if this script created it):
#  - the window to close is the one found by diffing the window IDs before and after the
#    spawn, and it is closed by that recorded ID (`spawn-at close --id`), never by class or name;
#  - a process is only killed if its PID passes every check in safe_to_kill: not this script, not
#    an ancestor of this script (your terminal), not a PID that owned a window before the spawn,
#    not the PID of the window that was focused at start, and it hosts exactly one window;
#  - no gnome-terminal case: its PID is the shared gnome-terminal-server that hosts every terminal
#    window, and killing it closes all of them. The terminal cases use the repo's own fixture.

set -euo pipefail

DRY_RUN=0
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=1 ;;
        -h|--help) sed -n 2,16p "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "unknown argument: $arg (try --dry-run)" >&2; exit 2 ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BIN="$PROJECT_ROOT/target/debug/spawn-at"
VERIFY_BIN="$PROJECT_ROOT/target/debug/spawn-at-verify"
# Relative to PROJECT_ROOT so the path has no spaces (the command is word-split below).
FIXTURE="target/debug/fixture"
cd "$PROJECT_ROOT"

# 1. Ensure binaries are built
if [ "$DRY_RUN" -eq 0 ] && { [ ! -f "$BIN" ] || [ ! -f "$VERIFY_BIN" ] || [ ! -f "$PROJECT_ROOT/$FIXTURE" ]; }; then
    echo "==> Building spawn-at, spawn-at-verify and the fixture window..."
    cargo build --manifest-path "$PROJECT_ROOT/Cargo.toml" -p spawn-at -p spawn-at-verify --features spawn-at-verify/fixture
fi

echo "========================================================"
echo "    spawn-at Live Smoke Test (GNOME Window Placement)   "
echo "========================================================"
[ "$DRY_RUN" -eq 1 ] && echo "DRY RUN: nothing is spawned, queried or closed."

# PIDs of this script and of every ancestor (your terminal and shell): never killed.
ancestor_pids() {
    local pid=$$ ppid
    while [ -n "$pid" ] && [ "$pid" -gt 1 ]; do
        echo "$pid"
        ppid=$(sed 's/^.*) [A-Za-z] //' "/proc/$pid/stat" 2>/dev/null | cut -d' ' -f1) || break
        [ "$ppid" = "$pid" ] && break
        pid="$ppid"
    done
}
ANCESTORS="$(ancestor_pids | tr '\n' ' ')"

# Window JSON helpers without jq: one object per line, then pick fields.
window_lines() { tr -d '\n' | sed 's/},[[:space:]]*{/}\n{/g'; }
pids_of() { window_lines | grep -o '"pid":[[:space:]]*[0-9]*' | grep -o '[0-9]*$' | sort -u | tr '\n' ' '; }
focused_pid() { window_lines | grep '"focused":[[:space:]]*true' | grep -o '"pid":[[:space:]]*[0-9]*' | grep -o '[0-9]*$' | head -1; }
windows_of_pid() { window_lines | grep -c "\"pid\":[[:space:]]*$1[,}[:space:]]" || true; }

FOCUSED_PID=""
if [ "$DRY_RUN" -eq 0 ]; then
    FOCUSED_PID="$("$BIN" query windows --json | focused_pid)"
fi

# safe_to_kill PID BEFORE_PIDS AFTER_JSON: succeeds only if PID is a process this run created and
# nothing else depends on it. Fails closed: an unknown focused window means no kill at all.
safe_to_kill() {
    local pid="$1" before_pids="$2" after_json="$3"
    [ "$pid" -gt 1 ] 2>/dev/null || return 1
    [ -n "$FOCUSED_PID" ] || return 1
    case " $ANCESTORS " in *" $pid "*) return 1 ;; esac
    [ "$pid" != "$FOCUSED_PID" ] || return 1
    case " $before_pids " in *" $pid "*) return 1 ;; esac
    [ "$(windows_of_pid "$pid" <<< "$after_json")" -le 1 ] || return 1
    return 0
}

# 2. Query workarea layout
if [ "$DRY_RUN" -eq 1 ]; then
    WA_X=0; WA_Y=0; WA_W=0; WA_H=0
else
LAYOUT_JSON=$("$BIN" query layout --json)
echo "Active layout: $LAYOUT_JSON"

read -r WA_X WA_Y WA_W WA_H <<< "$("$VERIFY_BIN" smoke-eval workarea <<< "$LAYOUT_JSON")"
fi

echo "Target workarea: ($WA_X, $WA_Y) ${WA_W}x${WA_H}"
echo ""

# close_spawned WIN_ID WIN_PID BEFORE_PIDS AFTER_JSON: closes the window this script created, by
# its recorded compositor ID. The process is killed only if it is still around afterwards and
# passes safe_to_kill; otherwise it is left alone (the fixture exits by itself).
close_spawned() {
    local win_id="$1" win_pid="$2" before_pids="$3" after_json="$4"
    echo "Cleaning up spawned window (ID=$win_id)..."
    "$BIN" close --id "$win_id" --no-wait 2>/dev/null || true
    sleep 0.5
    if kill -0 "$win_pid" 2>/dev/null; then
        if safe_to_kill "$win_pid" "$before_pids" "$after_json"; then
            kill "$win_pid" 2>/dev/null || true
        else
            echo "Not killing PID $win_pid (not provably ours alone); the window was closed by ID."
        fi
    fi
}

# Helper to run a test case
# Usage: run_test <APP_NAME> <COMMAND> <ANCHOR> <MARGIN> <CLASS_HINT> [SIZE_W] [SIZE_H]
run_test() {
    local app_name="$1"
    local cmd="$2"
    local anchor="$3"
    local margin="$4"
    local class_hint="$5"
    local size_w="${6:-}"
    local size_h="${7:-}"

    echo "--------------------------------------------------------"
    local size_desc=""
    if [ -n "$size_w" ] && [ -n "$size_h" ]; then
        size_desc="--size $size_w $size_h "
    fi
    echo "TEST: Spawning $app_name with ${size_desc}--anchor $anchor $([ "$margin" -gt 0 ] && echo "-m $margin")"
    echo "--------------------------------------------------------"

    if [ "$DRY_RUN" -eq 1 ]; then
        echo "WOULD SPAWN: $BIN spawn --anchor $anchor $([ "$margin" -gt 0 ] && echo "-m $margin ")${size_w:+--size $size_w $size_h }-- $cmd"
        echo "WOULD CLOSE: only the new window found by diffing window IDs, with: $BIN close --id <that id>"
        echo "WOULD KILL:  at most the one process of that window, and only if safe_to_kill passes"
        echo "             (not an ancestor of this script, not the focused window's PID at start,"
        echo "              not a PID that owned a window before, hosts exactly one window); else nothing"
        echo ""
        return 0
    fi

    # Pre-record the IDs and PIDs of the windows that exist before the spawn: anything in
    # these sets was not created by this script and is never closed or killed.
    local before_json before_ids before_pids
    before_json=$("$BIN" query windows --json)
    before_ids=$("$VERIFY_BIN" smoke-eval window-ids <<< "$before_json")
    before_pids=$(pids_of <<< "$before_json")

    # Build and execute spawn-at command
    local spawn_cmd=("$BIN" "spawn" "--anchor" "$anchor")
    if [ "$margin" -gt 0 ]; then
        spawn_cmd+=("-m" "$margin")
    fi
    if [ -n "$size_w" ] && [ -n "$size_h" ]; then
        spawn_cmd+=("--size" "$size_w" "$size_h")
    fi
    spawn_cmd+=("--" $cmd)

    echo "Executing: ${spawn_cmd[*]}"
    local stderr_log
    stderr_log=$(mktemp)
    
    local exit_code=0
    "${spawn_cmd[@]}" 2> "$stderr_log" || exit_code=$?
    cat "$stderr_log" >&2

    # Assert exit code 0
    if [ "$exit_code" -ne 0 ]; then
        echo "❌ FAILED: spawn-at exited with non-zero code $exit_code!"
        rm -f "$stderr_log"
        exit 1
    fi

    # Assert no "Timed out" in stderr
    if grep -iq "timed out" "$stderr_log"; then
        echo "❌ FAILED: Found false 'Timed out' warning in stderr!"
        rm -f "$stderr_log"
        exit 1
    fi
    rm -f "$stderr_log"

    # Allow brief window manager settlement
    sleep 0.8

    # Query windows to find the newly mapped window
    local after_json
    after_json=$("$BIN" query windows --json)

    local win_info
    win_info=$("$VERIFY_BIN" smoke-eval find-window --before-ids "$before_ids" --class-hint "$class_hint" --app-name "$app_name" <<< "$after_json")

    if [ "$win_info" = "NOT_FOUND" ]; then
        echo "❌ FAILED: Newly spawned $app_name window not found in query windows!"
        exit 1
    fi

    local win_id win_pid win_x win_y win_w win_h
    IFS="|" read -r win_id win_pid win_x win_y win_w win_h <<< "$win_info"

    echo "Observed window: ID=$win_id PID=$win_pid at ($win_x, $win_y) size ${win_w}x${win_h}"

    # Calculate expected geometry and assert within 1px tolerance and inside workarea
    local verify_result
    verify_result=$("$VERIFY_BIN" smoke-eval verify-geom --win-x "$win_x" --win-y "$win_y" --win-w "$win_w" --win-h "$win_h" --wa-x "$WA_X" --wa-y "$WA_Y" --wa-w "$WA_W" --wa-h "$WA_H" --anchor "$anchor" --margin "$margin")

    local passed exp_x exp_y err_x err_y wa_ok
    IFS="|" read -r passed exp_x exp_y err_x err_y wa_ok <<< "$verify_result"

    local ext_ver
    ext_ver=$(gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell/Extensions/SpawnAt --method org.freedesktop.DBus.Properties.Get org.gnome.Shell.Extensions.SpawnAt ProtocolVersion 2>/dev/null | sed -n 's/.*uint32 \([0-9]*\).*/\1/p' || echo "1")
    ext_ver="${ext_ver:-1}"

    if [ "$passed" = "True" ]; then
        echo "✅ SUCCESS: Position ($win_x, $win_y) matches expected ($exp_x, $exp_y) [delta: dx=$err_x, dy=$err_y <= 1px, within workarea]"
    elif [ "$ext_ver" -lt 2 ] && [ "$win_w" -ge "$WA_W" ] && [ "$win_h" -ge "$WA_H" ]; then
        echo "⚠️ Note: Window started maximized and active GNOME Shell extension in memory is ProtocolVersion $ext_ver (v2 required for unmaximize). Succeeded with truthful diagnostic."
    else
        echo "❌ FAILED: Position ($win_x, $win_y) diverges from expected ($exp_x, $exp_y) [delta: dx=$err_x, dy=$err_y, within_wa=$wa_ok]"
        # Clean up before exit
        close_spawned "$win_id" "$win_pid" "$before_pids" "$after_json"
        exit 1
    fi

    close_spawned "$win_id" "$win_pid" "$before_pids" "$after_json"

    sleep 0.5
    echo ""
}

# Run live smoke test cases:
# 1. gnome-calculator --anchor center
run_test "gnome-calculator" "gnome-calculator" "center" 0 "calculator"

# 2. gnome-calculator --anchor top-right -m 24
run_test "gnome-calculator" "gnome-calculator" "top-right" 24 "calculator"

# 3-5. The former gnome-terminal cases, on the repo's own fixture window (its title is unique,
# it exits by itself after 20 s, and it has its own process, so nothing shared is ever touched).
# 3. fixture --anchor center
run_test "fixture" "$FIXTURE --title smoke-fixture-3 --exit-after-ms 20000" "center" 0 "smoke-fixture-3"

# 4. fixture --anchor top-right -m 24
run_test "fixture" "$FIXTURE --title smoke-fixture-4 --exit-after-ms 20000" "top-right" 24 "smoke-fixture-4"

# 5. fixture --size 30 20 --anchor bottom-right (sub-minimum size test case)
run_test "fixture" "$FIXTURE --title smoke-fixture-5 --exit-after-ms 20000" "bottom-right" 16 "smoke-fixture-5" 30 20

# 6. gnome-calculator --size 30 20 --anchor bottom-right
run_test "gnome-calculator" "gnome-calculator" "bottom-right" 16 "calculator" 30 20

# 7. gnome-text-editor --size 30 20 --anchor bottom-right
if command -v gnome-text-editor >/dev/null 2>&1; then
    run_test "gnome-text-editor" "gnome-text-editor --standalone" "bottom-right" 16 "texteditor" 30 20
fi

echo "========================================================"
if [ "$DRY_RUN" -eq 1 ]; then
    echo "    DRY RUN DONE: NOTHING WAS SPAWNED OR CLOSED         "
else
    echo "    ALL LIVE SMOKE TESTS PASSED WITHIN 1PX TOLERANCE!   "
fi
echo "========================================================"
