#!/bin/sh
# psh-start: start a PRIVATE headless GNOME Shell in a fresh sandbox under /tmp (xflash-priv.XXXXXX).
# It never touches the live session: own D-Bus, own Wayland socket, own Xwayland, own keyfile
# gsettings, a copy of the extension. Run from anywhere; prints the sandbox path.
# Everything that runs inside it must go through `psh` (the guard wrapper next to this file).
set -eu
root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$root"
mkdir -p scratch/xflash02/priv
live_wl=${WAYLAND_DISPLAY:-}
live_x=${DISPLAY:-}
live_bus=${DBUS_SESSION_BUS_ADDRESS:-}
live_xdg=${XDG_RUNTIME_DIR:-}
before=$(ls /tmp/.X11-unix 2>/dev/null | tr '\n' ' ')
SB=$(mktemp -d /tmp/xflash-priv.XXXXXX)
mkdir -m 0700 "$SB/run"
mkdir -p "$SB/config/glib-2.0/settings" "$SB/data/gnome-shell/extensions/spawn-at@harsh.local" "$SB/cache" "$SB/state" "$SB/log"
cp -r assets/gnome/modern/. "$SB/data/gnome-shell/extensions/spawn-at@harsh.local/"
cat > "$SB/config/glib-2.0/settings/keyfile" <<KF
[org/gnome/shell]
enabled-extensions=['spawn-at@harsh.local']
disable-user-extensions=false
welcome-dialog-last-shown-version='99'
KF
# H7 replica (X11-FLASH-02h): the live session's other extensions (COPIES, read from the user's
# extension directory) and its allowlisted settings, minus GSConnect (network) and Win11Clip Bridge
# (clipboard). System extensions are found through XDG_DATA_DIRS.
if [ -n "${H7:-}" ]; then
  h=docs/evidence/x11-flash-02/data/h7
  list="'spawn-at@harsh.local'"
  for u in $(cat "$h/user-extensions.txt"); do
    cp -r "$HOME/.local/share/gnome-shell/extensions/$u" "$SB/data/gnome-shell/extensions/"
    list="$list, '$u'"
  done
  for u in $(cat "$h/system-extensions.txt"); do list="$list, '$u'"; done
  if [ -n "${H7_SKIP:-}" ]; then
    # H7_SKIP="uuid1 uuid2": leave those out of the enabled list (bisect by halves)
    nl="'spawn-at@harsh.local'"
    for u in $(cat "$h/user-extensions.txt" "$h/system-extensions.txt"); do
      case " $H7_SKIP " in *" $u "*) ;; *) nl="$nl, '$u'";; esac
    done
    list=$nl
  fi
  {
    printf "[org/gnome/shell]\nenabled-extensions=[%s]\ndisable-user-extensions=false\nwelcome-dialog-last-shown-version='99'\n\n" "$list"
    cat "$h/keyfile.ini"
  } > "$SB/config/glib-2.0/settings/keyfile"
fi
cat > "$SB/live.env" <<LV
LIVE_WAYLAND_DISPLAY='$live_wl'
LIVE_DISPLAY='$live_x'
LIVE_BUS='$live_bus'
LIVE_XDG_RUNTIME_DIR='$live_xdg'
LV
# The private environment. Variables that name the live session are unset, not just overridden.
cat > "$SB/env" <<EV
unset SESSION_MANAGER DESKTOP_STARTUP_ID XDG_ACTIVATION_TOKEN XDG_SESSION_ID XDG_SEAT XDG_VTNR \
      GNOME_DESKTOP_SESSION_ID NOTIFY_SOCKET JOURNAL_STREAM INVOCATION_ID MANAGERPID AT_SPI_BUS_ADDRESS \
      GNOME_SETUP_DISPLAY SYSTEMD_EXEC_PID DCONF_PROFILE XAUTHORITY MAINPID LISTEN_PID LISTEN_FDS
export PSH_SB='$SB'
export XDG_RUNTIME_DIR='$SB/run'
export XDG_CONFIG_HOME='$SB/config'
export XDG_DATA_HOME='$SB/data'
export XDG_CACHE_HOME='$SB/cache'
export XDG_STATE_HOME='$SB/state'
export GSETTINGS_BACKEND=keyfile
export GNOME_SHELL_SESSION_MODE=user
export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=GNOME
export SPAWN_AT_CONFIG='$SB/none.toml'
export DBUS_SESSION_BUS_ADDRESS='unix:path=$SB/run/bus'
export WAYLAND_DISPLAY=xflash-wl
EV
echo "$SB" > scratch/xflash02/priv/CURRENT
# shellcheck disable=SC1090
. "$SB/env"
dbus-daemon --session --address="unix:path=$SB/run/bus" --nofork --nopidfile > "$SB/log/dbus.log" 2>&1 &
echo $! > "$SB/dbus.pid"
i=0; while [ ! -S "$SB/run/bus" ] && [ $i -lt 50 ]; do sleep 0.1; i=$((i+1)); done
[ -S "$SB/run/bus" ] || { echo "private bus did not start" >&2; exit 1; }
"$root/docs/evidence/x11-flash-02/tools/target/debug/sysd1stub" > "$SB/log/sysd1stub.log" 2>&1 &
echo $! > "$SB/sysd.pid"
sleep 0.5
# systemd-run itself must reach the LIVE user manager (to create a memory-limited scope), so it gets
# the live bus; the Shell inside the scope sources the private environment first thing.
nohup env DBUS_SESSION_BUS_ADDRESS="$live_bus" XDG_RUNTIME_DIR="$live_xdg" \
  systemd-run --user --scope --quiet -p MemoryMax=3G -p MemorySwapMax=0 -- \
  sh -c '. "$1"; exec gnome-shell --headless --wayland --wayland-display=xflash-wl --virtual-monitor 1920x1080 --sm-disable' sh "$SB/env" \
  > "$SB/log/shell.log" 2>&1 &
echo $! > "$SB/shell.pid"
i=0; while [ ! -S "$SB/run/xflash-wl" ] && [ $i -lt 100 ]; do sleep 0.1; i=$((i+1)); done
sleep 1.5
if ! kill -0 "$(cat "$SB/shell.pid")" 2>/dev/null; then echo "private shell exited; see $SB/log/shell.log" >&2; exit 2; fi
# The private Xwayland: the new socket in /tmp/.X11-unix, and the authority file mutter wrote in the private runtime dir.
after=$(ls /tmp/.X11-unix 2>/dev/null | tr '\n' ' ')
newx=""
for s in $after; do case " $before " in *" $s "*) ;; *) newx="$s";; esac; done
auth=$(ls "$SB"/run/.mutter-Xwaylandauth.* 2>/dev/null | head -1 || true)
# Mutter logs "Using public X11 display :N, (using :M for managed services)": clients use N. The
# newest socket in /tmp/.X11-unix is M (the Shell's own display), which hangs ordinary clients.
disp=$(sed -n 's/.*Using public X11 display \(:[0-9]*\).*/\1/p' "$SB/log/shell.log" | head -1)
if [ -z "$disp" ] && [ -n "$newx" ]; then disp=":${newx#X}"; fi
if [ -n "$disp" ]; then echo "export DISPLAY=$disp" >> "$SB/env"; fi
if [ -n "$auth" ]; then echo "export XAUTHORITY='$auth'" >> "$SB/env"; fi
echo "$SB"
