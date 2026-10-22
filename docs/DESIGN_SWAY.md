# Design: a Sway backend

**Status:** implemented as an experimental, opt-in backend in `src/platform/linux/sway/`. Only
`SPAWN_AT_BACKEND=sway` selects it (`src/platform/linux/registry.rs`).

Every fact below is marked with how it was checked. "Verified" means observed on one machine with
Sway 1.9 running headless in a private directory (harness: [`scripts/sway-headless.sh`](../scripts/sway-headless.sh),
section 11), with the repo's own fixture window as the only client. No real application and no
GNOME session was involved. "Unverified" means nobody checked it.

## 1. Goal and non-goals

Goal: `SPAWN_AT_BACKEND=sway spawn-at spawn ...` places a new window with anchor, size and margin.
`query`, `close`, `focus` and `transform` work too, inside a Sway session, with no helper to install.

Not in the first version:

- Flicker-free placement (section 5 explains why it **cannot be promised**).
- Pointer queries, maximize and minimize.
- i3, which uses the same IPC (section 10, question 7).
- Automatic selection of the backend (section 7).
- Wayland compositors other than Sway.

## 2. Verified facts about Sway

Sway version tested: 1.9 (`sway --version` was run).

### The IPC socket and its frame

The socket is a unix socket named by `$SWAYSOCK`. A frame is the 6 bytes `i3-ipc`, a u32 payload
length and a u32 type, both in native byte order, then the payload. Replies use the same frame with
JSON payloads.

Evidence: `man sway-ipc` (section "MESSAGE AND REPLY FORMAT"), and a raw exchange with `socat`.
Sending `i3-ipc`, length 0, type 1 returned `i3-ipc`, `7a 05 00 00` (1402), `01 00 00 00` and
`[ { "id": 4, "type": "workspace" ...`.

### Message and event types

| Message type | Number |
| :-- | :-- |
| `RUN_COMMAND` | 0 |
| `GET_WORKSPACES` | 1 |
| `SUBSCRIBE` | 2 |
| `GET_OUTPUTS` | 3 |
| `GET_TREE` | 4 |
| `GET_MARKS` | 5 |
| `GET_BAR_CONFIG` | 6 |

The backend needs the first five. Event types have the high bit set:

| Event | Value |
| :-- | :-- |
| workspace | `0x80000000` |
| output | `0x80000001` |
| mode | `0x80000002` |
| window | `0x80000003` |

Evidence: `man sway-ipc`.

### A raw `RUN_COMMAND` with `nop` succeeds

It returned `[ { "success": true } ]`. Evidence: a `socat` exchange.

### Window events carry a `change` value

The values are `new`, `close`, `focus`, `title`, `fullscreen_mode`, `move`, `floating`, `urgent` and
`mark`. Evidence: `man sway-ipc`. Observed: `new`, `title`, `floating`, `focus` and `close`.

### `new` arrives when the view is mapped

At that moment the container has `rect` 0x0 and `name` null, and a `title` event follows. Evidence:
observed.

### `move position` is relative to the workspace, `move absolute position` to all outputs

`move position X Y` is relative to the workspace's usable area. `move absolute position X Y` is
relative to all outputs.

Evidence: `man 5 sway`, and observed with a 30 px bar. `move position 100 100` gave `rect.y = 130`,
`move absolute position 100 100` gave `y = 100`, and `move position 0 0` gave `y = 30`.

### Container `rect` is global, and the workspace `rect` is the usable area

The workspace `rect` excludes a bar. Evidence: observed with a second output at x=1920. `move absolute
position 2000 100` gave `rect.x = 2000`, and `move position 10 10` on that output gave `x = 1930`. With
a top bar the workspace `rect` was `y=30, height=510` of a 540 px output.

### With output scale 2 all coordinates are logical pixels

A 1920x1080 mode at scale 2 has `rect` 960x540, and the mode stays 1920x1080. Evidence: observed.

### A new window is tiled; `floating enable` centres it

A new window is tiled and fills the workspace. `floating enable` makes it floating at the centre of
the usable area, keeping the client's size (640x400 here). Evidence: observed.

### `kill` closes the client

`kill` on a container asks the client to close. The fixture exited and the node left the tree.
Evidence: observed.

### `for_window` over IPC keeps only the first command

A `for_window` rule registered over IPC **takes only the first command** of a comma list.

- `for_window [app_id="x"] floating enable, resize set 700 450, move position 300 200` made
  `floating enable` the rule and ran the other two immediately on the focused container.
- A quoted list was accepted, but the rule applied only `floating enable`.
- `for_window [..] mark NAME` works.
- `man 5 sway` says commands are separated by `,` or `;`, and criteria are kept across `,`.

Evidence: observed with the fixture, in two experiments.

### `create_output` makes a headless output

It creates a headless output over IPC, so a multi-output layout can be tested. Evidence: observed
(`HEADLESS-2`).

### Not verified

- The exact moment of the first frame, relative to `window::new`.
- A client that maps at a size different from its request.
- XWayland windows (disabled in the harness).
- Sway versions other than 1.9.
- Whether an i3 session behaves the same.

### Real JSON shapes (from the private Sway)

`GET_OUTPUTS` is an array. These are the fields we use; the rest is in `man sway-ipc`:

```json
{ "name": "HEADLESS-1", "active": true, "scale": 1.0, "transform": "normal",
  "rect": {"x":0,"y":0,"width":1920,"height":1080},
  "current_mode": {"width":1920,"height":1080,"refresh":0},
  "current_workspace": "1", "focus": [4] }
```

`GET_WORKSPACES` is an array of objects like this one. The `rect` is the usable area (a 30 px bar gave
`y=30,height=510`).

```json
{"id":4,"type":"workspace","name":"1","num":1,"output":"HEADLESS-1","focused":true,"visible":true,
 "rect":{"x":0,"y":0,"width":1920,"height":1080}}
```

`GET_TREE` is `root`, then `output` nodes (a pseudo output `__i3` holds `__i3_scratch`), then
`workspace` nodes, then containers. A tiled window is in `nodes` and a floating one in
`floating_nodes`. A window, a leaf with `pid`, looks like this:

```json
{ "id": 5, "type": "con" /* "floating_con" once floating */,
  "name": "sway-harness-1", "pid": 1874654,
  "app_id": "dev.spawnat.swayharness", "shell": "xdg_shell",
  "visible": true, "focused": true,
  "rect": {"x":0,"y":0,"width":1920,"height":1080},
  "window_rect": {"x":0,"y":0,"width":1920,"height":1080},
  "deco_rect": {"x":0,"y":0,"width":0,"height":0},
  "geometry": {"x":0,"y":0,"width":640,"height":400},
  "border": "none", "window_properties": null }
```

After `[con_id=5] floating enable, resize set 800 500, move position 100 150` the `type` is
`floating_con`, the `rect` is `{100,150,800,500}`, the `window_rect` is `{0,0,800,500}` and the
`border` is `"csd"`. The `geometry` is still `640x400`: it is the client's own size and lags the
container until the client commits.

For an XWayland window `app_id` is null and `window_properties` holds `class`, `instance` and `title`.
This comes from `man sway-ipc` and is unverified here, because XWayland is disabled in the harness.

A `RUN_COMMAND` reply is an array with one `{"success": true}` per command, or
`{"success": false, "parse_error": true, "error": "..."}`.

A `window` event looks like `{"change":"new","container":{<the node shape above>}}`. For one window
placed by a rule the order was `new`, `title`, `floating`, `focus`, and later `close`.

## 3. Mapping the traits

How the backend fills the platform traits, which are described in
[DESIGN_PLATFORMS.md](DESIGN_PLATFORMS.md), section 4.

| Method | What the Sway driver does |
| :-- | :-- |
| `arm` | Subscribes to `window` events and records the windows that exist, before the child starts. |
| `disarm` | Drops the subscription. `spawn` never reaches it, because `LinuxBackend` does not forward it. |
| `post_spawn` | The placement step: waits for the new window, then places it. Runs in every mode, `--no-wait` too. |
| `prepare_claim_wait` | Returns nothing. The pipeline polls instead. |
| `get_windows` | Reads `GET_TREE` and lists the window leaves. |
| `get_monitors`, `get_workareas`, `get_layout` | Read `GET_OUTPUTS` and `GET_WORKSPACES` (below). |
| `get_window_rect` | The trait default, built on `get_windows`. |
| `get_cursor_position` | Unsupported: Sway IPC has no pointer query. |
| `transform_window` | The same commands as placing a new window (section 4). |
| `focus_window` | `[con_id=N] focus` |
| `close_window` | `[con_id=N] kill` |
| `set_window_state`, `defocus_window` | Unsupported (maximize, minimize, restore, defocus). |
| `resolve_id` | The command name, or the explicit class. Sway matches by `app_id`. |
| `protocol_version`, helper status | No helper, so there is nothing to check. |
| `provisioner` | None. `install` prints "No installation required for Sway." and `uninstall` prints "No uninstallation required for Sway." |
| `Host` | Unchanged. |
| Update hooks and session actions | Unchanged: they are the GNOME ones and do nothing unless `XDG_CURRENT_DESKTOP` contains GNOME. The Sway backend records no session action. |

Why `post_spawn` does the placement: it is awaited in every mode, and a `--no-wait` run never awaits
a claim waiter. The X11 driver has the same shape.

How `arm` is ordered: it subscribes first and takes the snapshot second, so no window can appear
unseen between the two.

### Windows

A window is a `con` or `floating_con` leaf that has a pid, an app id or X11 properties. Its fields:

- `id` is the container id. `pid` is the pid. `title` is `name`.
- `class` is `app_id`, or `window_properties.class` for XWayland.
- `x`, `y`, `w`, `h` are the container `rect`.
- `maximized` is always false. `minimized` is true only for a window on the scratchpad (`__i3_scratch`),
  which has no monitor and no workspace.
- `monitor` is the index of the output, and `workspace` is the workspace number.
- `state` is `normal` or `fullscreen`, the words GNOME uses. Floating and tiled are both `normal`.

### Monitors and work areas

`GET_OUTPUTS` gives the screen rectangle (`rect`, in logical pixels) and `scale`. `GET_WORKSPACES`
gives the work area of each output's current workspace; its `rect` is the usable area, so bars are
already excluded. Sway has no primary monitor, so the first active output is flagged `primary`, as the
X11 driver does. The default monitor is then the same "primary" as on GNOME.

### The cursor

`get_cursor_position` returns `UnsupportedCapability`, so `query pointer` fails with the standard
message. `spawn --anchor cursor` and `--monitor cursor` do not fail: `spawn` **uses (0, 0) in place of
the pointer**.

## 4. What the backend declares, and placement

What the backend declares:

| Field | Value | Why |
| :-- | :-- | :-- |
| `runtime_transform` | true | `floating enable`, `resize set` and `move absolute position` work on a mapped window (verified). |
| `claim_wait` | false | There is no claim waiter. |
| `close` | true | `kill` closes the fixture (verified). |
| `reveal_control` | false | Sway cannot hold a new window invisible until it is placed (section 5). |
| `needs_helper` | false | Plain IPC. |

Not supported: cursor position (no IPC), maximize and minimize (Sway has fullscreen and the
scratchpad, which are not the same thing), defocus (no meaning) and flicker-free placement.

### How a window is placed

The shared engine, `spawn-at-core::geometry`, computes the target rectangle from the work area it is
given. For Sway:

1. The work area is the `rect` of the target output's current workspace (verified to be the usable
   area, in logical pixels, in global coordinates). The screen area is the output `rect`. `scale` is
   only reported: Sway's coordinates are already logical, so no scaling math is needed.
2. The default output is the first active output, in the order `GET_OUTPUTS` lists them. `--monitor N`
   picks the Nth in that order. The shared `resolve_monitor_from_layout` does the choosing.
3. With `--size`, one chained `RUN_COMMAND` carries the criteria across the commas
   (`man 5 sway`): `[con_id=N] floating enable, resize set W H, move absolute position X Y`. The size
   comes first, so the position is the last thing applied. `move absolute` is used because the
   engine's coordinates are global (verified on two outputs).
4. Without `--size`, the window keeps the size its client chose. The driver sends `floating enable`,
   waits up to 1 s for the container to report a size that stays the same for two reads 40 ms apart,
   then sends the move.
5. `--size` means the whole container (`rect`), as on GNOME. `resize set` counts only the content, so
   after the chained command the driver reads the container back once. If the size differs, it asks
   once for the content size reduced by the border and title bar (`rect` minus `window_rect`).
   Verified with `for_window ... border normal 3`: `--size 600 400` gave a container of exactly 600x400
   (content 594x372).
6. If a window has a minimum size, Sway raises a smaller request (75x50 for the floating fixture
   window, verified). The driver then computes the position again with the size the window really has,
   so an anchor like bottom-right keeps it inside the area. `--size 30 20` at bottom-right with margin
   16 gives 75x50 at (1829, 1014), the position `verify_and_report_placement` expects.
7. With exclusive focus, a `focus` command follows. The `Leave` focus intent **cannot be honoured**,
   because Sway focuses a new window by its own rules.

`transform` on an existing window uses the same commands. It always sends a resize to the target
rectangle's size, even when `--size` was not given.

## 5. Flicker-free strategy

There is **no way over IPC to run commands before the first frame** that is both reliable and safe. These
are the options that were weighed:

| Option | How | Verdict |
| :-- | :-- | :-- |
| A. `for_window` rule before launch | Register a rule, then start the app. | Rejected as the mechanism. |
| B. React to `window::new` | Subscribe in `arm`; on the matching `new` event, send one chained command. | **Chosen.** |
| C. Start, then poll and move | `exec`, poll `GET_TREE`, then `move`. | Rejected: B, but slower. |
| D. Start on a hidden workspace or output, then move and show | Workspace games. | Rejected: it changes the user's layout, needs cleanup and breaks focus. |

### Why not A

A rule runs inside Sway at map time, so it has no race. But:

- Over IPC only the first command of the list belongs to the rule (verified), and a quoted list
  applies only `floating enable` (verified).
- Rules cannot be removed, so they accumulate in the compositor.
- Rules are global, so two spawns of the same app collide.
- The `app_id` must be known in advance, and spawn-at only knows what the user typed, which may
  differ.
- A `pid` criterion would be exact, but the pid does not exist yet.

A rule is usable only for `mark`.

### Why B, and its limit

The event is emitted when the view is mapped (observed: the node has `rect` 0x0 then). The command
comes from another process, so whether it lands before the first frame is unverified and cannot be
guaranteed. One frame at the tiled position is possible.

So the backend keeps `reveal_control` false and the docs say that a flash can happen. The
subscription is made in `arm`, so no event is missed: the socket buffers events between `arm` and
`post_spawn`.

**The real flash has not been measured.** The flash analysis in `crates/spawn-at-verify` is based on the
GNOME extension. It would need a Sway source of frame timing, for example `wlr-screencopy`; whether
that is feasible is unverified. Until it is measured, the docs and `--help` must not promise
flicker-free placement on Sway.

## 6. Matching a launched process to its window

`window::new` gives `container.pid` (the pid of the Wayland client, from the socket credentials;
verified equal to the fixture's pid), `app_id` and `id`. Only windows that were not in the snapshot
from `arm` are considered. The rules, in order:

1. The window's pid is the launched pid.
2. The window's pid is a descendant of the launched pid. This covers wrapper scripts, `env` and
   launchers that exec.
3. The window's `app_id` (or XWayland class) matches the entry's hint. This covers launchers that
   hand over to a running server, where the pid never matches.
4. Nothing matches within 5 seconds: **no other window is moved**. The standard "post-spawn hook failed"
   warning says the application was started but not placed, and `spawn` carries on.

Details of rule 3:

- The hint is matched against the class only. The title is not passed, so a window titled like the
  command is not taken.
- An empty hint or `*` never matches by rule 3.
- A rule 3 window is taken only after a 300 ms grace, in which a pid match may still arrive. The first
  rule 3 window is the one kept.

The 5 seconds are a constant (`place::WINDOW_WAIT`), the same wait the X11 driver uses. `post_spawn`
has no access to `window_timeout`.

## 7. Selection: never automatic

`LinuxBackend::bootstrap` walks `registry::candidates`. `SPAWN_AT_BACKEND` is compared, lowercased
and not trimmed, to `x11` and `sway`, and any other value is ignored.

`SPAWN_AT_BACKEND=sway` (any case, nothing trimmed, like `x11`) selects Sway. If the value is `sway`,
`candidates` returns Sway alone. Every other input returns what it returned before Sway was added.

`XDG_CURRENT_DESKTOP=sway` without the variable **no longer gets X11 silently**. `registry::unsupported`
refuses it with the "not supported" error (Leak 11 in DESIGN_PLATFORMS.md), unless the session type is
`x11`.

### The proof

- The existing table test (9 forced values, 14 desktops, D-Bus up or down, against a copy of the old
  selection) is unchanged and still passes.
- The Sway cases are three separate tests that say why they differ from that copy:
  - `sway`, `SWAY`, `Sway` and `sWaY` select Sway for every desktop and D-Bus result, and never ask for
    the GNOME connection.
  - ` sway`, `sway `, `swayx` and six more near values agree with the old copy.
  - Sway is never a candidate without the variable.
- 19 tests run the built binary against a stand-in socket (`tests/sway_backend_cli_test.rs`).

### Forced but unusable is an error

If `SPAWN_AT_BACKEND=sway` and the Sway socket cannot be used, `bootstrap` returns
`DriverError::IpcError` and nothing after it runs. The socket cannot be used when `SWAYSOCK` is unset or
empty, when it is not connectable, or when the first reply is not valid IPC.

`main` prints the message as "Driver initialization error: ..." and exits 1. For an unset `SWAYSOCK`
the message is `SPAWN_AT_BACKEND=sway, but $SWAYSOCK is not set. Run spawn-at inside a Sway session, or
unset SPAWN_AT_BACKEND.`

**There is no fallback to X11.** The GNOME candidate does fall back, silently, when D-Bus is unreachable.
`I3SOCK` is not consulted (section 10, question 7: no i3 backend).

## 8. IPC: hand-roll, no new dependency

The IPC is written by hand with `std::os::unix::net::UnixStream` and `serde_json`, which spawn-at
already depends on.

- The protocol is a 14-byte header plus JSON (section 2, verified by hand with `socat`). Encoding and
  decoding are pure functions, unit-tested with bytes recorded from the harness.
- Blocking I/O is wrapped in `tokio::task::spawn_blocking`, the pattern the X11 driver uses, so there is
  no async runtime coupling.
- The module is `src/platform/linux/sway/`. It exists only on Linux, so the Windows and macOS trees are
  untouched.

The cost of the alternative crates (`swayipc`, `swayipc-async`, `i3ipc`) **was not measured**. The
machine had no network, and none of them was in the local cargo registry, so `cargo tree` could not
resolve them. For reference, spawn-at's own normal tree on Linux has 145 distinct entries (counted
with `cargo tree --edges normal` on 2026-10-20), and `serde`, `serde_json` and `thiserror` are
already in it. An async crate would probably add an async I/O stack; that is unverified. Measure it
with `cargo add --dry-run` and `cargo tree` before reconsidering.

## 9. Tests

- Unit tests for the codec, the JSON conversions, the command strings and the matching rules, with JSON
  fixtures in `tests/fixtures/sway/`. They run everywhere CI runs, with no Sway.
- Driver tests against a scripted socket (`fake.rs`, `tests_write.rs`): `bootstrap` errors, `arm` with
  `post_spawn` matching by pid, by descendant and by app id, the timeout, `close`, `focus`.
- 19 tests that run the built binary against a recording stand-in socket.
- `scripts/sway-live.sh`: live checks on a private headless Sway, run by hand. The last recorded run
  passed 62 checks and failed none ([TEST_PLAN.md](TEST_PLAN.md) section 7.2).
- No live test is gated by an environment variable, and no CI job installs `sway`.
- Flash measurement is out of scope for the first version (section 5).

## 10. Open questions for the owner (answered)

Seven questions were put to the owner before the build. The answers were applied in the
implementation.

| Question | Decision |
| :-- | :-- |
| 1. Is a possible one-frame flash acceptable? | Yes, for a first release. Sway is documented as experimental and `reveal_control` stays false. |
| 2. Should Sway be picked automatically? | No. Only `SPAWN_AT_BACKEND=sway` selects it. |
| 3. Does `--size` mean the container or the content? | The whole container, as on GNOME, so the same command gives the same result. |
| 4. Which output is the default? | The "primary" one, which for Sway is the first output. |
| 5. May CI install `sway`? | No CI job installs it. |
| 6. Map maximize and minimize? | No. They are unsupported in the first version. |
| 7. An i3 backend? | No. |

## 11. The harness

`scripts/sway-headless.sh` has the commands `start`, `env`, `status` and `stop`, and the options
`--dry-run` and `--extra-config FILE`.

- It starts `sway` with `WLR_BACKENDS=headless`, `WLR_LIBINPUT_NO_DEVICES=1` and `WLR_RENDERER=pixman`,
  so no GPU is touched. The config has one `HEADLESS-1` 1920x1080 output, no bar and no XWayland.
- Sway runs in a private `0700` temp directory that is also its own `XDG_RUNTIME_DIR`. Its Wayland and
  IPC sockets are created there and nowhere else. `WAYLAND_DISPLAY`, `DISPLAY`, `SWAYSOCK` and
  `I3SOCK` are removed from its environment, so it can never nest into another compositor.
- The environment is set only on the child, or printed for the caller's subshell. Nothing goes to
  `systemctl --user set-environment`, `~/.profile` or any file outside the private directory.
- The PID is recorded in `sway.pid` in that directory. `stop` kills only that PID, after checking that
  `/proc/PID` is a `sway` whose `XDG_RUNTIME_DIR` is that directory and whose `WLR_BACKENDS` is
  `headless`. Otherwise it refuses and removes nothing. It then removes the directory, which must carry
  the marker file, be owned by you, match `*/spawn-at-sway.*` and not be the real runtime dir.
- `--dry-run` prints what `start` or `stop` would do and does nothing.

The author's recorded checks: a recorded PID that was a `sleep` was refused and left alive;
`stop --dir /tmp` was refused; a normal `stop` left no sway, swaybar or fixture process and no
directory; `gnome-shell` kept its PID and command line; no journal line mentioned spawn-at, sway or an
assertion during the runs.

To try it yourself (not run in your session by the author):

```bash
scripts/sway-headless.sh start --dry-run
eval "$(scripts/sway-headless.sh start)"            # prints the exports; use a subshell or a spare terminal
swaymsg -t get_outputs                              # talks to the private sway only
WAYLAND_DISPLAY="$WAYLAND_DISPLAY" target/debug/fixture --protocol wayland --title demo --exit-after-ms 60000 &
swaymsg -t get_tree
scripts/sway-headless.sh stop --dir "$SPAWN_AT_SWAY_DIR"
```

## 12. Implementation status

No new dependency: `Cargo.toml` and `Cargo.lock` are unchanged. The module exists only on Linux, so the
three non-Linux `cargo check` runs and the CI job `check-other-os` do not see it.

| File in `src/platform/linux/sway/` | Role |
| :-- | :-- |
| `ipc.rs` | The 14-byte frame: encode, decode, limits, errors. Pure. |
| `types.rs` | Serde types for outputs, workspaces, the tree and `window` events; conversion to `WindowMetadata` and `MonitorLayout`. Pure. |
| `command.rs` | The `RUN_COMMAND` strings. Integers only: nothing the user typed reaches Sway's command line. Pure. |
| `matching.rs` | The pid, descendant and app id rules, with the grace and the snapshot. Pure. |
| `client.rs` | A blocking connection with timeouts, and the `$SWAYSOCK` messages. |
| `place.rs` | Subscribe and snapshot, wait for the window, place it, correct the frame, single-window commands. |
| `mod.rs` | `SwayDriver`: one short connection per call. The only state is the subscription `arm` leaves for `post_spawn`. |
| `fake.rs`, `tests_write.rs` | Test only: a scripted IPC server and the driver tests against it. |

What differs from the first design (sections 1 to 8):

- The default monitor is the first output, as on GNOME (section 10, question 4).
- `--size` is the whole container. The correction after `resize set` and the re-anchoring of a raised
  window are additions (section 4, items 5 and 6).
- `state` in `query windows` is `normal` or `fullscreen`, the GNOME words, not `floating` and `tiled`.
- A window in the scratchpad (`__i3_scratch`) is listed with `minimized` true, no monitor and no
  workspace.
- There is no `window_timeout`: `post_spawn` waits a fixed 5 s (section 6). `--no-wait` still waits
  for the placement, because the pipeline awaits `post_spawn` in every mode.
- `FocusIntent::Exclusive` adds `focus`. `Leave` cannot be honoured, because Sway focuses a new window
  by its own rules.
- Only the first entry of a batch is placed, and the rest are ignored without a message. The X11
  driver does the same.
- Maximize, minimize, restore and defocus return the standard "does not support this feature" error.
  `--anchor cursor` and `--monitor cursor` use (0, 0), because the pointer cannot be read.

What was run: everything on the private headless Sway 1.9 of `scripts/sway-headless.sh`, with the
repo's fixture window as the only client. The GNOME session was not touched, and `gnome-shell` kept
its PID. By hand, while building: the same placements through `spawn-at spawn --json`, `close` and
`transform`; a real border and title bar; `--no-wait`; forced Sway with `SWAYSOCK` unset.

Not verified, and not claimed:

- A real Sway session or real applications.
- XWayland windows (disabled in the harness).
- Sway versions other than 1.9, and i3.
- Frame timing before the first paint: the one-frame flash of section 5 has not been measured.
- `--area screen` and `--area unbounded`.
- Clients that map at a size other than the one asked.
- Scales other than 1 and 2, and more than two outputs.
