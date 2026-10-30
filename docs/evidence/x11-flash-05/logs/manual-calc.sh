#!/bin/sh
# manual-calc.sh: the Task B side tests that are not cells. Every app process is launched through
# `psh` (private session) and every pid is recorded here; windows are closed with `spawn-at close`.
# Output is plain text so it can be pasted into the evidence.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
T="$root/docs/evidence/x11-flash-03/tools"
CLI="/home/harsh/Documents/GITHUB PROJECTS/spawn-at/scratch/xflash05/target-fix/debug/spawn-at"
psh() { sh "$T/psh" "$@"; }
q() { psh "$CLI" query windows --json; }
win_geom() { q | jq -c '.[] | {id,pid,class,w,h,x,y}'; }
close_all() {
  for w in $(q | jq -r '.[].id'); do psh "$CLI" close --id "$w" >/dev/null 2>&1; done
  sleep 1
  for p in "$@"; do [ -n "$p" ] && kill -0 "$p" 2>/dev/null && kill -TERM "$p" 2>/dev/null; done
  sleep 1
}

echo "== T1 saved window-size 900x700, launch WITHOUT spawn-at, read after 2 s =="
psh gsettings set org.gnome.calculator window-size '(900, 700)'
psh gsettings get org.gnome.calculator window-size
p1=$(psh sh -c 'gnome-calculator >/dev/null 2>&1 & echo $!')
echo "launched pid $p1"
sleep 2
win_geom
w1=$(q | jq -r '.[0].id // empty')
echo "== T2 close it; does the app write window-size? =="
[ -n "$w1" ] && psh "$CLI" close --id "$w1"
sleep 2
psh gsettings get org.gnome.calculator window-size
kill -0 "$p1" 2>/dev/null && kill -TERM "$p1" 2>/dev/null
sleep 1

echo "== T3 relaunch WITHOUT spawn-at; is 900x700 restored? =="
p2=$(psh sh -c 'gnome-calculator >/dev/null 2>&1 & echo $!')
echo "launched pid $p2"
sleep 2
win_geom
w2=$(q | jq -r '.[0].id // empty')

echo "== T4 size sent 2 s AFTER map handling (transform of the same window) =="
if [ -n "$w2" ]; then psh "$CLI" transform --id "$w2" --size 900 700 --anchor bottom-right -m 24 --json; fi
sleep 0.5
win_geom
sleep 2
win_geom
close_all "$p2"

echo "== T4b control: launch at natural size, transform 2 s after map =="
p3=$(psh sh -c 'GDK_BACKEND=x11 GSETTINGS_BACKEND=memory gnome-calculator >/dev/null 2>&1 & echo $!')
sleep 2
win_geom
w3=$(q | jq -r '.[0].id // empty')
if [ -n "$w3" ]; then psh "$CLI" transform --id "$w3" --size 900 700 --anchor bottom-right -m 24 --json; fi
sleep 0.5
win_geom
sleep 2
win_geom
close_all "$p3"

echo "== T5 spawn-at run, then a transform correction 100 ms after it returns =="
psh "$CLI" spawn -c org.gnome.Calculator --no-focus --anchor bottom-right --size 900 700 -m 24 --json -- gnome-calculator 2>&1 | tail -2
sleep 0.3
win_geom
w5=$(q | jq -r '.[0].id // empty')
if [ -n "$w5" ]; then psh "$CLI" transform --id "$w5" --size 900 700 --anchor bottom-right -m 24 --json; fi
sleep 1
win_geom
close_all
echo "== done =="
