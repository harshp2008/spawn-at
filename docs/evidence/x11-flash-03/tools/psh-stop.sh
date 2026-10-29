#!/bin/sh
# psh-stop: stop ONLY the processes recorded in the sandbox, copy its logs to scratch/, remove it.
# Usage: psh-stop.sh [LABEL]   (LABEL names the log copy; default "priv")
root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$root"
cur=scratch/xflash02/priv/CURRENT
[ -f "$cur" ] || exit 0
SB=$(cat "$cur")
[ -d "$SB" ] || { rm -f "$cur"; exit 0; }
label=${1:-priv}
out="scratch/xflash02/priv/logs-$label-$(date +%H%M%S)"
mkdir -p "$out"
cp "$SB"/log/* "$out"/ 2>/dev/null
for f in shell.pid sysd.pid dbus.pid; do
  p=$(cat "$SB/$f" 2>/dev/null) || continue
  if [ -n "$p" ] && kill -0 "$p" 2>/dev/null; then kill -TERM "$p" 2>/dev/null; fi
done
sleep 2
# Helpers the private Shell started (ibus, gvfs, evolution, at-spi, ...): only processes whose own
# environment carries this sandbox's tag (PSH_SB=<sandbox>); pids are read from /proc, never matched by name.
for d in /proc/[0-9]*; do
  p=${d#/proc/}
  if [ -r "$d/environ" ] && tr "\0" "\n" < "$d/environ" 2>/dev/null | grep -qx "PSH_SB=$SB"; then kill -TERM "$p" 2>/dev/null; fi
done
sleep 2
for d in /proc/[0-9]*; do
  p=${d#/proc/}
  if [ -r "$d/environ" ] && tr "\0" "\n" < "$d/environ" 2>/dev/null | grep -qx "PSH_SB=$SB"; then kill -KILL "$p" 2>/dev/null; fi
done
for f in shell.pid sysd.pid dbus.pid; do
  p=$(cat "$SB/$f" 2>/dev/null) || continue
  if [ -n "$p" ] && kill -0 "$p" 2>/dev/null; then kill -KILL "$p" 2>/dev/null; fi
done
# Remove the sandbox only if it is a plain directory under /tmp/xflash-priv.* and not a mount point.
case "$SB" in /tmp/xflash-priv.*) ;; *) echo "psh-stop: unexpected sandbox path $SB, not removing" >&2; exit 1;; esac
if findmnt -M "$SB" >/dev/null 2>&1; then echo "psh-stop: $SB is a mount point, not removing" >&2; exit 1; fi
if findmnt -rn -o TARGET | grep -q "^$SB/"; then echo "psh-stop: a mount (gvfs/portal fuse) is still under $SB, not removing" >&2; exit 1; fi
rm -rf "$SB"
rm -f "$cur"
echo "stopped; logs in $out"
