#!/usr/bin/env bash
# scripts/focus-chain-sway.sh
# Runs the focus-return chain test (crates/spawn-at-verify/tests/tui/focus_chain_sway.rs) against a
# private headless Sway that scripts/sway-headless.sh starts and stops. It never touches your
# desktop session. Needs sway and a build of spawn-at and the fixture (it builds them if missing).
#
# Usage: scripts/focus-chain-sway.sh
set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
HARNESS="$SCRIPT_DIR/sway-headless.sh"
cd "$PROJECT_ROOT"

cargo build --locked -p spawn-at -p spawn-at-verify --features spawn-at-verify/fixture,spawn-at-verify/tui || exit 2
cargo test --locked -p spawn-at-verify --features tui,fixture --test tui_test --no-run || exit 2

DIR=""
stop_session() {
    if [ -n "$DIR" ]; then
        "$HARNESS" stop --dir "$DIR" || echo "focus-chain: could not stop the private sway in $DIR" >&2
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
    *) echo "focus-chain: SWAYSOCK is outside the private dir; stopping" >&2; exit 1 ;;
esac
echo "==> private sway in $DIR"

env -u DISPLAY -u XDG_CURRENT_DESKTOP -u DBUS_SESSION_BUS_ADDRESS \
    SPAWN_AT_BACKEND=sway SWAYSOCK="$SWAYSOCK" WAYLAND_DISPLAY="$WAYLAND_DISPLAY" \
    XDG_SESSION_TYPE=wayland \
    cargo test --locked -p spawn-at-verify --features tui,fixture --test tui_test \
    focus_chain_sway -- --ignored --nocapture --test-threads=1 &
child=$!
forward() { kill -TERM "$child" 2>/dev/null; }
trap forward INT TERM HUP
while :; do
    wait "$child"
    code=$?
    kill -0 "$child" 2>/dev/null || break
done
echo "==> exited with $code"
exit "$code"
