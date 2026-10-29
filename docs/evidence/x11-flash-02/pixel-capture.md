# X11-FLASH-02i-b pixel capture: attempts and results

Private headless Shell, DING not loaded. All calls went through `tools/psh` on the private D-Bus
bus; nothing ran on the live session. Time-box: 3 attempts.

## Attempt 1: introspect `org.gnome.Shell.Screencast`

Command:

```
tools/psh gdbus introspect --session --dest org.gnome.Shell.Screencast \
  --object-path /org/gnome/Shell/Screencast
```

Result: **success**. The private Shell exports
`org.gnome.Shell.Screencast` with `Screencast(s file_template, a{sv} options) -> (b, s)`,
`ScreencastArea(i x, i y, i width, i height, s file_template, a{sv} options) -> (b, s)`,
`StopScreencast() -> (b)`, the `Error(s, s)` signal, and the read-only property
`ScreencastSupported = true`. Full text: `scratch/xflash02/cap2-introspect.txt`.

## Attempt 2: record 5 s at 60 fps of a solid-colour fixture window

Commands (fixture from the repo, launched through `psh`; no `spawn-at` placement):

```
mkdir -p scratch/xflash02/cap2
tools/psh gdbus call --session --dest org.gnome.Shell.Screencast \
  --object-path /org/gnome/Shell/Screencast \
  --method org.gnome.Shell.Screencast.Screencast \
  "$PWD/scratch/xflash02/cap2/cap" "{'framerate': <uint32 60>}"
# then, after 1 s:
timeout --signal=TERM --kill-after=2 8 tools/psh "$PWD/target/debug/fixture" \
  --protocol x11 --state-colors --width 640 --height 420 --title xflash-pixel \
  --exit-after-ms 6000
# then StopScreencast
```

Result: **failed at the first call**, before any frame was written. Exact error:

```
Error: GDBus.Error:org.freedesktop.DBus.Error.Failed: GDBus.Error:org.freedesktop.DBus.Error.Failed: Failed to start screen cast: Couldn't connect pipewire context
```

`StopScreencast` answered `(false,)`. No capture file was created; `scratch/xflash02/cap2/`
contains only the three logs (`start.log`, `fixture.log` empty, `stop.log`). The fixture itself ran
and exited 0 (`--exit-after-ms 6000`), so the window existed while the capture was requested; the
failure is in the private session's PipeWire connection (`XDG_RUNTIME_DIR` points into the sandbox,
which has no PipeWire socket).

## Attempt 3

Not run: attempt 2 failed, and the prompt ends pixel capture when an attempt fails. The private
Shell did not crash (pid 3008087, start 82421156, still running); the live Shell pid 2718897 /
start 81373971 was unchanged.
