# Running the batches in an isolated GNOME session: feasibility and bisect plan

X11-FLASH-02e2, step 2. Read-only: nothing was started, installed or written outside the repository.
Tags: MEASURED (a command was run read-only, or a file was read), INFERRED, HYPOTHESIS (needs a
trial). A statement tagged HYPOTHESIS is not known to work.

## 1. What the project already has (a)

- MEASURED: `scripts/sway-headless.sh` starts a private headless Sway and prints exports; there is
  no equivalent for GNOME in `scripts/` (`ls scripts`: five Sway or comment scripts, `smoke-live.sh`
  which drives the real desktop, and `check-doc-coverage.sh`). `.github/workflows` has no
  headless or nested GNOME job (grep for `nested` and `--headless`: no hit).
- MEASURED: `docs/DESIGN_RUNNER_SETUP.md` is about tests of an existing window in the verify
  runner, not about a private session.
- MEASURED: `docs/TEST_LOG.md` 6.12 ("Nested shell, report only") says `gnome-shell --headless
  --wayland --no-x11 --wayland-display=NAME --virtual-monitor 1920x1080` started on GNOME 46 inside
  `dbus-run-session` in about 1 s and got its own D-Bus; "the spawn-at extension was not shown to
  load there"; and an incident: a dconf write from the private session reached the real database
  because the D-Bus-activated dconf service did not inherit `XDG_CONFIG_HOME`. Its advice is
  `GSETTINGS_BACKEND=keyfile` plus `XDG_CONFIG_HOME` exported before `dbus-run-session`, and a
  file log source, since the verify tool reads `journalctl --user`.

## 2. What this machine offers (b)

MEASURED (`--help`, `command -v`, `ls`):

| Item | Result |
| :-- | :-- |
| `gnome-shell` 46.0 options | `--wayland`, `--nested`, `--no-x11`, `--wayland-display`, `--display-server`, `--headless`, `--virtual-monitor`, `--sm-disable` |
| `mutter`, `Xwayland`, `dbus-run-session`, `dbus-daemon`, `xdotool`, `systemd-run`, `loginctl` | present |
| `Xvfb`, `xvfb-run`, `weston`, `machinectl`, `podman`, `virt-manager`, `multipass`, `vagrant` | absent |
| `qemu-system-x86_64` | present; `/dev/kvm` is `root:kvm` with an ACL entry giving the user rw, so KVM works without a group change |
| `docker` | the binary is present; the user is in `sudo` but not in `docker`, so it needs root |
| GPU | `/dev/dri` is present; `libegl-mesa0` and `libegl1` are installed. Whether a headless Shell gets an EGL context (hardware or llvmpipe) is HYPOTHESIS until tried |
| Mutter / Shell | `mutter 46.2-1ubuntu0.24.04.18`, `gnome-shell 46.0` |

Do not pass `--no-x11`: the measurement needs Xwayland, which the Shell starts itself.

## 3. Isolation design (c)

Goal: nothing mutable shared with the real session. Everything below is created under one
`SB=$(mktemp -d /tmp/w11c-sandbox.XXXXXX)` with a cleanup trap that runs `findmnt -M "$SB"` (must
fail) before any `rm -rf`, as the user's global rules require.

| Variable or path | Value | Why |
| :-- | :-- | :-- |
| `XDG_RUNTIME_DIR` | `$SB/run`, mode 0700 | own Wayland socket, own D-Bus sockets, own `pipewire`/portal sockets |
| `WAYLAND_DISPLAY` | `xflash-wl` (the Shell's `--wayland-display`) | cannot collide with `wayland-0` |
| `DBUS_SESSION_BUS_ADDRESS` | set by `dbus-run-session` | own session bus: the extension's D-Bus name, `SetLogging` and `spawn-at` calls reach only the private Shell |
| `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME`, `XDG_STATE_HOME` | `$SB/config`, `$SB/data`, `$SB/cache`, `$SB/state` | no write to `~/.config`, `~/.cache`, `~/.local` |
| `GSETTINGS_BACKEND` | `keyfile` (file `$XDG_CONFIG_HOME/glib-2.0/settings/keyfile`) | no dconf service, so nothing can reach the real dconf database (the 6.12 incident) |
| Extension | copy of the installed directory into `$XDG_DATA_HOME/gnome-shell/extensions/`; enabled by writing `enabled-extensions=['spawn-at@harsh.local']` and `disable-user-extensions=false` into the private keyfile under `[org/gnome/shell]` | the real `enabled-extensions` is never read or written |
| `SPAWN_AT_CONFIG` | a path that does not exist (as in TEST_LOG 6.23) | the CLI does not read `~/.config/spawn-at` |
| `HOME` | unchanged (apps need it), but apps use `GSETTINGS_BACKEND=memory` and fresh profiles | no dconf or profile writes |

Shared resources that remain, and what neutralises them:

- `/tmp/.X11-unix` and the abstract X socket namespace: Xwayland picks the first free display
  number, so it does not take the real session's `:0`; the real server stays reachable by name only
  if a client is given `DISPLAY=:0`, which the private environment never does. Residual risk: low.
- journald: the private Shell is not a systemd user unit, so its output goes to a file
  (`2> $SB/shell.log`) and not to the user journal. Tools must read that file (see section 5).
- PipeWire and the clipboard: the private runtime dir has no PipeWire socket; the private Shell has
  its own clipboard. Not shared.
- Snap apps (Firefox, gedit snap): confinement expects the real `/run/user/1000`; exclude them
  from the private session, or use Chrome with a fresh `--user-data-dir`. HYPOTHESIS that Chrome
  works there.
- `~/.cache` fontconfig and mesa shader caches: redirected by `XDG_CACHE_HOME`.
- CPU and memory: shared. A 3 GB `systemd-run --user --scope` limit stays in place.
- A crash of the private Shell takes down only its process tree; the real Shell is untouched.
  MEASURED risk to name: a crash dump may still be written by apport into `/var/crash`.

## 4. Alternatives, ranked by risk and effort (d)

| Rank | Option | State here | Effort | Isolation |
| :-- | :-- | :-- | :-- | :-- |
| 1 | Private headless Shell in `dbus-run-session` (this file) | all tools present | low | process level, shared kernel and GPU |
| 2 | VM with `qemu-system-x86_64` and KVM | binary and `/dev/kvm` present; no guest image installed | high (an Ubuntu 24.04 image and a desktop install, tens of minutes and several GB) | full |
| 3 | Container with a headless Shell (`docker`) | binary present, root needed, no image | medium | process and filesystem, shared kernel |
| 4 | Second Linux user with a systemd user session | needs a new account and root | medium | user level; not installed or created |
| 5 | Nested compositor inside the real Shell (`--nested`) | possible | low | weak: it is a window of the real Shell and shares its X and D-Bus |

## 5. Recommendation and smoke test (e)

Start with rank 1. The smoke test is xterm only, 3 runs. Command lines (not executed; shown as a
script outline to be run by the maintainer's go-ahead):

```
SB=$(mktemp -d /tmp/w11c-sandbox.XXXXXX); trap 'findmnt -M "$SB" >/dev/null || rm -rf "$SB"' EXIT
mkdir -m 0700 "$SB/run"; mkdir -p "$SB/config/glib-2.0/settings" "$SB/data/gnome-shell/extensions" "$SB/cache" "$SB/state"
cp -r ~/.local/share/gnome-shell/extensions/spawn-at@harsh.local "$SB/data/gnome-shell/extensions/"
printf "[org/gnome/shell]\nenabled-extensions=['spawn-at@harsh.local']\ndisable-user-extensions=false\n" > "$SB/config/glib-2.0/settings/keyfile"
env XDG_RUNTIME_DIR="$SB/run" XDG_CONFIG_HOME="$SB/config" XDG_DATA_HOME="$SB/data" XDG_CACHE_HOME="$SB/cache" \
    XDG_STATE_HOME="$SB/state" GSETTINGS_BACKEND=keyfile SPAWN_AT_CONFIG="$SB/none" \
    dbus-run-session -- sh -c '
      gnome-shell --headless --wayland --wayland-display=xflash-wl --virtual-monitor 1920x1080 2> "$SB/shell.log" &
      echo $! > "$SB/shell.pid"; sleep 4
      # DISPLAY of the private Xwayland: read from the newest socket in /tmp/.X11-unix (HYPOTHESIS, a trial decides)
      export WAYLAND_DISPLAY=xflash-wl DISPLAY=:N
      docs/evidence/x11-flash-02/tools/run-batch.sh --app xterm --probe --out "$SB/b_smoke" --shell-pid-file "$SB/shell.pid" --shell-log "$SB/shell.log"'
```

What to watch: `kill -0 $(cat $SB/shell.pid)` between runs; the strings `crashed with signal`,
`assertion`, `Bail out` in `$SB/shell.log`; the extension answering `gdbus call ... SetLogging`
on the private bus; `spawn-at query windows` listing the private windows only; the Wayland socket
`$SB/run/xflash-wl`. A crash is a nonzero `kill -0` or any of those strings.

Changes needed in `tools/` (inside `docs/evidence/x11-flash-02/tools/` only; none made yet):
- `run.rs`: `health()` reads the Shell pid from `--shell-pid-file` when given, instead of
  `pgrep -x gnome-shell`; the extension log comes from `--shell-log FILE` instead of `journalctl`,
  and the `[spawn-at-time]` prefix is searched for anywhere in a line, because stderr lines may carry
  a prefix (HYPOTHESIS; a trial shows the real format).
- The stray-process scan, the state file and the progress file are per `--out`, so they already
  separate the sessions. `DISPLAY`, `WAYLAND_DISPLAY` and the bus address are inherited from the
  environment, so `observe`, `gdbus` and `spawn-at` follow the private session without a change.
- `analyze.rs`: none. `run-batch.sh`: none.
- Needs a trial: whether the Shell loads the copied extension without a GNOME session manager;
  whether Xwayland starts headless; how `DISPLAY` is learned; whether the extension's log reaches
  stderr in a usable format.

## 6. Bisect plan (f), run inside the isolated session, safest first

Each cell is 3 runs unless noted; the observer is attached only where stated. "Crash" means the
private Shell aborts with the assertion. H numbers are from `crash-2026-10-29-second.md`.

| # | Cell | If it crashes | If clean |
| :-- | :-- | :-- | :-- |
| 6 | xterm, spawn-at, observer (control, 80 clean runs already on the live Shell) | the private Shell is itself unstable: H5 | expected; the setup works |
| 1 | gnome-text-editor native Wayland, plain launch, graceful close | H1/H5 (the app alone, no X11, no tool) | expected |
| 2 | native Wayland, `spawn-at spawn`, no observer | H4 for the Wayland path or the startup token | extension alone is safe on Wayland |
| 3 | forced X11, plain launch, no spawn-at, no observer | H2: GTK-on-X11 alone is enough; our tools exonerated | continue |
| 4 | forced X11 with `spawn-at spawn`, no observer | H4 or the startup id (section 9 of the crash note), because 3 was clean | continue |
| 4b | as 4 but the app argv starts with `env -u DESKTOP_STARTUP_ID -u XDG_ACTIVATION_TOKEN` | if 4 crashed and 4b is clean: the spawn-at startup id is the cause | if 4b also crashes the token is not the cause |
| 5 | as 4 with the observer attached | if 4 was clean and 5 crashes: H3 | the observer is exonerated |
| 7 | close variants on forced X11 with the window focused and unfocused: `spawn-at close --id`, `WM_DELETE_WINDOW` sent by a small x11rb tool, and the app's own quit key (sent only inside the private session) | the variant that crashes names the trigger (H6) | a variant that never crashes is the safe close |

Order: 6, 1, 2, 3, 4, 4b, 5, 7. Stop at the first crash, record the private `shell.log` lines and
read the table; every later cell then needs the maintainer's decision. Expected outcomes by
hypothesis: H1 predicts crashes in 1, 3, 4, 5 and not 6 (even forced eog differs); H2 predicts
3, 4, 5 and not 1, 2; H3 predicts only 5; H4 predicts only 2 and 4 (not 3); H6 predicts the crash
follows the close in every crashing cell and depends on the variant in 7; H5 predicts 6 itself
crashes.

Cells 1 to 3 are the cheapest and settle whether our tools matter at all.
