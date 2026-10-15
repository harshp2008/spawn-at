#!/usr/bin/env bash
# scripts/verify-sway.sh
# Runs `spawn-at-verify run` against a private, headless Sway that scripts/sway-headless.sh starts
# and stops. It never touches your desktop session.
#
# Usage: scripts/verify-sway.sh [--dry-run] [-- <arguments for `spawn-at-verify run`>]
#   --dry-run  print what would be started and run; start and run nothing.
#   With no arguments after `--` it runs the specs tagged `headless-safe` (`--tag headless-safe`).
#   Example: scripts/verify-sway.sh -- --glob 'focus/*' --dir path/to/verify
#
# Needs: sway (and swaymsg). Uses target/debug/spawn-at, target/debug/spawn-at-verify and
# target/debug/fixture; builds them if missing.
#
# Safety (a window or process is only touched if the runner created it, see
# docs/DESIGN_RUNNER_SETUP.md section 3 and 4.3):
#  - sway runs only through scripts/sway-headless.sh (own runtime dir, own sockets, headless, no
#    XWayland); the runner gets SWAYSOCK and WAYLAND_DISPLAY of that private session and
#    SPAWN_AT_BACKEND=sway, and DISPLAY, XDG_CURRENT_DESKTOP and DBUS_SESSION_BUS_ADDRESS removed,
#    so it can reach neither an X server nor the GNOME extension of your session;
#  - the script refuses to start the runner if SWAYSOCK is not inside the private directory, and
#    the runner makes the same check itself and blocks every test otherwise;
#  - the fixture windows are launched, closed by id and stopped by the runner through the process
#    handle it holds; this script kills no process but the sway it started (by the recorded PID);
#  - on Ctrl-C or SIGTERM the runner closes what it opened and exits 130; sway is then stopped.

set -uo pipefail

DRY_RUN=0
RUN_ARGS=()
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=1 ;;
        -h|--help) sed -n 2,23p "${BASH_SOURCE[0]}"; exit 0 ;;
        --) shift; RUN_ARGS=("$@"); break ;;
        *) echo "unknown argument: $1 (try --dry-run, or put run arguments after --)" >&2; exit 2 ;;
    esac
    shift
done
[ ${#RUN_ARGS[@]} -gt 0 ] || RUN_ARGS=(--tag headless-safe)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
HARNESS="$SCRIPT_DIR/sway-headless.sh"
BIN="$PROJECT_ROOT/target/debug/spawn-at"
VERIFY="$PROJECT_ROOT/target/debug/spawn-at-verify"
FIXTURE="$PROJECT_ROOT/target/debug/fixture"
cd "$PROJECT_ROOT"

echo "========================================================"
echo "   spawn-at-verify on a private headless Sway           "
echo "========================================================"

if [ "$DRY_RUN" -eq 1 ]; then
    echo "DRY RUN: nothing is started or run."
    echo
    echo "Would start a private session with scripts/sway-headless.sh:"
    "$HARNESS" start --dry-run
    echo
    echo "Then run, with SPAWN_AT_BACKEND=sway SWAYSOCK=<private> WAYLAND_DISPLAY=<private> and"
    echo "DISPLAY, XDG_CURRENT_DESKTOP, DBUS_SESSION_BUS_ADDRESS unset:"
    printf '  %q run --bin %q' "$VERIFY" "$BIN"
    printf ' %q' "${RUN_ARGS[@]}"
    echo
    echo "Then stop the private sway with: scripts/sway-headless.sh stop --dir <private dir>"
    exit 0
fi

if [ ! -f "$BIN" ] || [ ! -f "$VERIFY" ] || [ ! -f "$FIXTURE" ]; then
    echo "==> Building spawn-at, spawn-at-verify and the fixture window..."
    cargo build --manifest-path "$PROJECT_ROOT/Cargo.toml" -p spawn-at -p spawn-at-verify --features spawn-at-verify/fixture || exit 2
fi

DIR=""
stop_session() {
    if [ -n "$DIR" ]; then
        "$HARNESS" stop --dir "$DIR" || echo "verify-sway: could not stop the private sway in $DIR" >&2
        DIR=""
    fi
}
trap stop_session EXIT
trap 'exit 130' INT TERM

out="$("$HARNESS" start)" || exit 1
eval "$out"
DIR="$SPAWN_AT_SWAY_DIR"
case "$SWAYSOCK" in
    "$DIR"/*) ;;
    *) echo "verify-sway: SWAYSOCK is outside the private dir; stopping" >&2; exit 1 ;;
esac
echo "==> private sway in $DIR"

# The runner, aimed at the private session only. Not exec: the trap must still stop sway.
env -u DISPLAY -u XDG_CURRENT_DESKTOP -u DBUS_SESSION_BUS_ADDRESS \
    SPAWN_AT_BACKEND=sway SWAYSOCK="$SWAYSOCK" WAYLAND_DISPLAY="$WAYLAND_DISPLAY" \
    XDG_SESSION_TYPE=wayland \
    "$VERIFY" run --bin "$BIN" "${RUN_ARGS[@]}"
code=$?
echo "==> spawn-at-verify exited with $code"
exit "$code"
