#!/usr/bin/env bash
# scripts/sway-headless.sh
# A private, headless Sway for testing the Sway backend. It never touches your desktop session.
#
#   scripts/sway-headless.sh start [--dry-run] [--extra-config FILE]   start sway, print the exports
#   scripts/sway-headless.sh env   [--dir DIR]            print the exports again (for a subshell)
#   scripts/sway-headless.sh status [--dir DIR]           say whether the recorded sway is running
#   scripts/sway-headless.sh stop  [--dir DIR] [--dry-run]  stop only the sway this script started
#
# Use the exports in a subshell, never in your login shell:
#   eval "$(scripts/sway-headless.sh start)"        # prints exports; run it in a subshell
#   ( eval "$(scripts/sway-headless.sh env)"; swaymsg -t get_outputs )
#
# Isolation (what makes this safe next to a running GNOME session):
#  - sway runs with WLR_BACKENDS=headless (no real display, no input devices, pixman renderer)
#    and with its OWN XDG_RUNTIME_DIR inside a private 0700 temp directory, so its Wayland socket
#    and its IPC socket are created there and nowhere else; WAYLAND_DISPLAY and DISPLAY are
#    removed from its environment, so it cannot open a window in another compositor (it is never
#    nested); XWayland is disabled in its config.
#  - the environment is set only on the child process, or printed for the caller's subshell. Nothing
#    is written to systemd's user environment, ~/.profile or any other file; the only files are
#    in the private temp directory.
#  - stop kills only the PID recorded at start, and only after checking that /proc/PID is a sway
#    whose XDG_RUNTIME_DIR is that private directory.

set -euo pipefail

MARKER=".spawn-at-sway-harness"

die() { echo "sway-headless: $*" >&2; exit 1; }

DRY_RUN=0
EXTRA=""
DIR="${SPAWN_AT_SWAY_DIR:-}"
CMD="${1:-}"
[ -n "$CMD" ] || die "usage: $0 start|env|status|stop [--dir DIR] [--dry-run]"
shift
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=1 ;;
        --dir) DIR="${2:-}"; shift ;;
        --extra-config) EXTRA="${2:-}"; [ -f "$EXTRA" ] || die "--extra-config: no such file: $EXTRA"; shift ;;
        -h|--help) sed -n 2,26p "${BASH_SOURCE[0]}"; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
    shift
done

# --- guards -----------------------------------------------------------------------------------

# A directory is ours if it has the marker, is owned by this user, and is not the real runtime dir.
check_dir() {
    local d="$1"
    [ -n "$d" ] || die "no harness directory: pass --dir DIR or set SPAWN_AT_SWAY_DIR (start printed it)"
    [ -d "$d" ] && [ -f "$d/$MARKER" ] || die "'$d' is not a directory made by this script (no $MARKER)"
    [ "$(stat -c %u "$d")" = "$(id -u)" ] || die "'$d' is not owned by you"
    [ "$d" != "${XDG_RUNTIME_DIR:-/nonexistent}" ] || die "refusing: '$d' is the real XDG_RUNTIME_DIR"
    case "$d" in */spawn-at-sway.*) ;; *) die "refusing: '$d' does not look like a harness directory" ;; esac
}

# True if /proc/PID is a sway running with the private directory as its runtime dir.
is_our_sway() {
    local pid="$1" d="$2"
    [ -r "/proc/$pid/comm" ] && [ "$(cat "/proc/$pid/comm")" = "sway" ] || return 1
    tr '\0' '\n' < "/proc/$pid/environ" 2>/dev/null | grep -qx "XDG_RUNTIME_DIR=$d" || return 1
    tr '\0' '\n' < "/proc/$pid/environ" 2>/dev/null | grep -qx "WLR_BACKENDS=headless" || return 1
    return 0
}

find_socket() { ls "$1"/sway-ipc.*.sock 2>/dev/null | head -1; }
find_wayland() { ls "$1"/wayland-* 2>/dev/null | grep -v '\.lock$' | head -1; }

print_env() {
    local d="$1" sock wl
    sock="$(find_socket "$d")"; wl="$(find_wayland "$d")"
    [ -n "$sock" ] && [ -n "$wl" ] || die "no sockets in '$d': is it running? (status)"
    # WAYLAND_DISPLAY as an absolute path: clients keep their own XDG_RUNTIME_DIR and still reach
    # this sway. DISPLAY is unset so a client cannot fall back to an X server of the real session.
    printf 'export SPAWN_AT_SWAY_DIR=%q\n' "$d"
    printf 'export SWAYSOCK=%q\n' "$sock"
    printf 'export WAYLAND_DISPLAY=%q\n' "$wl"
    printf 'export WLR_BACKENDS=headless\n'
    printf 'unset DISPLAY\n'
}

# --- commands ---------------------------------------------------------------------------------

case "$CMD" in
start)
    command -v sway >/dev/null || die "sway is not installed"
    if [ "$DRY_RUN" -eq 1 ]; then
        echo "DRY RUN: would create a private 0700 directory ${TMPDIR:-/tmp}/spawn-at-sway.XXXXXX with:"
        echo "  config      one headless output HEADLESS-1 1920x1080 at 0,0 scale 1, no bar, no XWayland"
        echo "  command     env -u WAYLAND_DISPLAY -u DISPLAY XDG_RUNTIME_DIR=<dir> WLR_BACKENDS=headless \\"
        echo "              WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman setsid sway -c <dir>/config"
        echo "  records     the PID of that sway in <dir>/sway.pid"
        echo "  prints      export SPAWN_AT_SWAY_DIR / SWAYSOCK / WAYLAND_DISPLAY (absolute paths in <dir>), unset DISPLAY"
        echo "  never       systemctl --user set-environment, ~/.profile, or any file outside <dir>"
        exit 0
    fi
    base="${TMPDIR:-/tmp}"
    [ -d "$base" ] && [ -w "$base" ] || die "'$base' is not writable"
    DIR="$(mktemp -d "$base/spawn-at-sway.XXXXXX")"
    chmod 700 "$DIR"
    : > "$DIR/$MARKER"
    [ "$DIR" != "${XDG_RUNTIME_DIR:-/nonexistent}" ] || { rm -rf "$DIR"; die "private dir equals the real runtime dir"; }
    cat > "$DIR/config" <<'EOF'
# Minimal config for the spawn-at test harness: one headless 1920x1080 output, nothing else.
output HEADLESS-1 resolution 1920x1080 position 0 0 scale 1
xwayland disable
focus_follows_mouse no
default_border none
default_floating_border none
EOF
    # Extra sway config lines from the caller (a bar, a scale, ...), appended to the private config.
    if [ -n "$EXTRA" ]; then { echo; echo "# --extra-config $EXTRA"; cat "$EXTRA"; } >> "$DIR/config"; fi
    # The child gets its own runtime dir and no way to reach the real compositors.
    env -u WAYLAND_DISPLAY -u DISPLAY -u SWAYSOCK -u I3SOCK \
        XDG_RUNTIME_DIR="$DIR" WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
        setsid sway -c "$DIR/config" < /dev/null > "$DIR/sway.log" 2>&1 &
    pid=$!
    echo "$pid" > "$DIR/sway.pid"
    ok=0
    for _ in $(seq 1 50); do
        if [ -n "$(find_socket "$DIR")" ] && [ -n "$(find_wayland "$DIR")" ]; then ok=1; break; fi
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.2
    done
    if [ "$ok" -ne 1 ]; then
        echo "sway-headless: sway did not come up; its log:" >&2; sed -n 1,20p "$DIR/sway.log" >&2
        if is_our_sway "$pid" "$DIR"; then kill "$pid" 2>/dev/null || true; fi
        rm -rf "$DIR"
        exit 1
    fi
    is_our_sway "$pid" "$DIR" || { kill "$pid" 2>/dev/null || true; die "started process is not the expected private sway; stopped it"; }
    # Never accept a socket outside the private directory.
    case "$(find_socket "$DIR")" in "$DIR"/*) ;; *) kill "$pid" 2>/dev/null || true; die "IPC socket is outside the private dir" ;; esac
    echo "sway-headless: started sway PID $pid in $DIR" >&2
    print_env "$DIR"
    ;;
env)
    check_dir "$DIR"
    print_env "$DIR"
    ;;
status)
    check_dir "$DIR"
    pid="$(cat "$DIR/sway.pid" 2>/dev/null || true)"
    if [ -n "$pid" ] && is_our_sway "$pid" "$DIR"; then echo "running: sway PID $pid in $DIR"; else echo "not running ($DIR)"; exit 1; fi
    ;;
stop)
    check_dir "$DIR"
    pid="$(cat "$DIR/sway.pid" 2>/dev/null || true)"
    if [ "$DRY_RUN" -eq 1 ]; then
        echo "DRY RUN: would stop recorded PID '${pid:-none}' only if /proc/PID is sway with XDG_RUNTIME_DIR=$DIR and WLR_BACKENDS=headless,"
        echo "         then remove $DIR (it has the $MARKER marker and is owned by you)"
        exit 0
    fi
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
        is_our_sway "$pid" "$DIR" || die "PID $pid is not the sway started for $DIR; not killing it, not removing the directory"
        kill -TERM "$pid"
        for _ in $(seq 1 50); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
        if kill -0 "$pid" 2>/dev/null; then
            is_our_sway "$pid" "$DIR" || die "PID $pid changed identity; not killing it"
            kill -KILL "$pid"
        fi
        echo "sway-headless: stopped sway PID $pid" >&2
    else
        echo "sway-headless: recorded PID '${pid:-none}' is not running" >&2
    fi
    rm -rf "$DIR"
    echo "sway-headless: removed $DIR" >&2
    ;;
*)
    die "unknown command '$CMD' (start, env, status, stop)"
    ;;
esac
