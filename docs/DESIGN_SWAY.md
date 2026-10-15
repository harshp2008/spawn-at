# Design: a Sway backend

Status: **implemented as an EXPERIMENTAL, opt-in backend** (`src/platform/linux/sway/`, branch `feat/sway-backend`); section 12 says what was built, where it differs from this design and what was run. The design (sections 1 to 11) was written on 2026-10-15 against `main` at `b45df0f` (the platform-neutral restructuring, see [DESIGN_PLATFORMS.md](DESIGN_PLATFORMS.md)); where the implementation chose differently, the text below is marked **Changed in the implementation** and section 12 has the reason. What is checked is marked **verified** with how; what is not is marked **unverified**.

The harness that produced the evidence is [`scripts/sway-headless.sh`](../scripts/sway-headless.sh) (section 11). Everything "verified" below was observed on this machine with Sway 1.9 running headless in a private directory, with the repo's own fixture window as the only client. No real application and no GNOME session was involved.

## 1. Goal and non-goals

Goal: `SPAWN_AT_BACKEND=sway spawn-at spawn ...` places a new window with anchor, size and margin, and `query`, `close`, `focus` and `transform` work, inside a Sway session, with no helper to install.

Non-goals of the first version: flicker-free placement (section 5 explains why it cannot be promised), pointer queries, maximize/minimize, i3 (same IPC, see open question 7), auto-selection (section 7), Wayland compositors other than Sway.

## 2. Verified facts about Sway

| Fact | How verified |
| :-- | :-- |
| Sway 1.9 is installed (`sway --version`). | Ran it. |
| IPC is a unix socket named by `$SWAYSOCK`. Frame: the 6 bytes `i3-ipc`, a u32 payload length, a u32 type, both in **native byte order**, then the payload; replies use the same frame with JSON payloads. | `man sway-ipc` (section "MESSAGE AND REPLY FORMAT"), and a raw exchange with `socat`: sending `i3-ipc`, length 0, type 1 returned `i3-ipc` + `7a 05 00 00` (1402) + `01 00 00 00` + `[ { "id": 4, "type": "workspace" ...`. |
| Message types: RUN_COMMAND 0, GET_WORKSPACES 1, SUBSCRIBE 2, GET_OUTPUTS 3, GET_TREE 4, GET_MARKS 5, GET_BAR_CONFIG 6 (the first five are the ones we need). Event types have the high bit set: workspace `0x80000000`, output `0x80000001`, mode `0x80000002`, **window `0x80000003`**. | `man sway-ipc`. |
| A raw `RUN_COMMAND` with payload `nop` returned `[ { "success": true } ]`. | `socat` exchange. |
| `window` events carry `change` = `new`, `close`, `focus`, `title`, `fullscreen_mode`, `move`, `floating`, `urgent`, `mark`. | `man sway-ipc`; `new`, `title`, `floating`, `focus`, `close` observed. |
| `move position X Y` is relative to the **workspace's usable area**; `move absolute position X Y` is relative to all outputs. | `man 5 sway`; observed: with a 30 px bar, `move position 100 100` gave `rect.y = 130`, `move absolute position 100 100` gave `y = 100`, `move position 0 0` gave `y = 30`. |
| `rect` of a container is in **global layout coordinates**, and the workspace `rect` is the **usable area** (it excludes a bar). | Observed with a second output at x=1920: `move absolute position 2000 100` gave `rect.x = 2000`; `move position 10 10` on that output gave `x = 1930`; with a top bar the workspace rect was `y=30, height=510` of a 540 px output. |
| With output scale 2 all coordinates are **logical pixels**: a 1920x1080 mode at scale 2 has `rect` 960x540, and the mode stays 1920x1080. | Observed. |
| A new window is **tiled** and fills the workspace; `floating enable` makes it floating at the **centre of the usable area**, keeping the client's size (640x400 here). | Observed. |
| `kill` on a container asks the client to close; the fixture exited and the node left the tree. | Observed. |
| `for_window` registered over IPC takes only the **first** command of a comma list: `for_window [app_id="x"] floating enable, resize set 700 450, move position 300 200` made `floating enable` the rule and ran the other two **immediately on the focused container**. A quoted list was accepted but the rule applied only `floating enable`. `for_window [..] mark NAME` works. `man 5 sway` says commands are separated by `,` or `;`, and criteria are kept across `,`. | Observed with the fixture (two experiments). |
| `create_output` creates a headless output over IPC, so a multi-output layout can be tested. | Observed (`HEADLESS-2`). |
| Sway sends `window::new` when the view is mapped; at that moment the container has `rect` 0x0 and `name` null; a `title` event follows. | Observed. |

Not verified: the exact moment of the first frame relative to `window::new`; behaviour with a client that maps at a size different from its request; XWayland windows (disabled in the harness); Sway versions other than 1.9; whether an `i3` session behaves the same.

### Real JSON shapes (from the private Sway)

`GET_OUTPUTS` (array; fields we use, the rest is in `man sway-ipc`):

```json
{ "name": "HEADLESS-1", "active": true, "scale": 1.0, "transform": "normal",
  "rect": {"x":0,"y":0,"width":1920,"height":1080},
  "current_mode": {"width":1920,"height":1080,"refresh":0},
  "current_workspace": "1", "focus": [4] }
```

`GET_WORKSPACES` (array): `{"id":4,"type":"workspace","name":"1","num":1,"output":"HEADLESS-1","focused":true,"visible":true,"rect":{"x":0,"y":0,"width":1920,"height":1080}, ...}`. The `rect` is the usable area (a 30 px bar gave `y=30,height=510`).

`GET_TREE`: `root` > `output` nodes (a pseudo output `__i3` holding `__i3_scratch`) > `workspace` nodes > containers. A tiled window is in `nodes`, a floating one in `floating_nodes`. A **window** (a leaf with `pid`) looks like:

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

After `[con_id=5] floating enable, resize set 800 500, move position 100 150`: `type` `floating_con`, `rect` `{100,150,800,500}`, `window_rect` `{0,0,800,500}`, `geometry` still `640x400` (the client's own size; it lags the container until the client commits), `border` `"csd"`. For an XWayland window `app_id` is null and `window_properties` holds `class`, `instance`, `title` (from `man sway-ipc`, **unverified** here because XWayland is disabled in the harness).

`RUN_COMMAND` reply: an array with one `{"success": true}` per command, or `{"success": false, "parse_error": true, "error": "..."}`.

`window` event: `{"change":"new","container":{<same node shape as above>}}`. Observed order for one window placed by a rule: `new`, `title`, `floating`, `focus`, later `close`.

## 3. Mapping the traits (DESIGN_PLATFORMS.md section 4)

The trait names are the ones in the code at `b45df0f`: `Capabilities` (`src/platform/capability.rs:26`), `WindowQuery` (`:101`), `WindowControl` (`:115`), `ReadySignal` (`:142`), `Provisioner` (`src/platform/provisioner.rs:12`), `Host` (`src/platform/host.rs:14`), `UpdateHooks` (`src/platform/update_hooks.rs:8`), `SessionActions` (`src/platform/session_actions.rs:9`), `Driver` (`crates/spawn-at-core/src/driver.rs:132`).

| Trait / method | Sway implementation |
| :-- | :-- |
| `Driver::arm(Batch)` | Connect to `$SWAYSOCK`, open a second connection and `SUBSCRIBE ["window"]` **before the child is started** (the spawn pipeline calls `arm` before launching, `src/commands/spawn.rs:285`), snapshot the ids already in `GET_TREE`, keep both in the driver. Return `Armed` with no launch environment (Sway needs no token; `XDG_ACTIVATION_TOKEN` is not used). |
| `Driver::disarm` | Drop the subscription. |
| `CompositorBackend::post_spawn(child_pid, batch)` | The placement step. It is awaited in every mode, including `--no-wait` (`spawn.rs:343`), which is why placement lives here and not in a claim waiter (a `--no-wait` run never awaits the waiter). Waits for the matching `window::new` (section 6), then issues one chained `RUN_COMMAND` (section 4). Same shape as the X11 driver (`src/platform/linux/x11/mod.rs:394`). |
| `ReadySignal::prepare_claim_wait` | Returns `None` in the first version; the pipeline then polls (`wait_for_spawn`, `src/commands/mod.rs:191`). A claim waiter fed by the same event stream can come later. |
| `WindowQuery::get_windows` | `GET_TREE`, flatten the leaves that have a `pid`. `WindowMetadata` (`src/target.rs:69`): `id` = `con` id, `pid`, `title` = `name`, `class` = `app_id`, or `window_properties.class` for XWayland, `app_id`, `x y w h` = `rect`, `focused`, `maximized` = false, `minimized` = false, `monitor` = index of the output, `workspace` = `num`, `state` = `fullscreen`/`floating`/`tiled`. |
| `get_monitors`, `get_workareas`, `get_layout` | `GET_OUTPUTS` for the screen rect (`rect`, logical pixels) and `scale`; `GET_WORKSPACES` for the work area of each output's current workspace (its `rect` is the usable area, so bars are already excluded). `MonitorInfo` (`crates/spawn-at-core/src/geometry.rs:71`) gets `name`, `scale`, `screen`, `workarea`. **Changed in the implementation:** the first active output is flagged `primary` (Sway has none; the X11 driver does the same), so the default monitor is the same "primary" as on GNOME; and `state` uses the GNOME words `normal` and `fullscreen` (floating and tiled are both `normal`). |
| `get_window_rect(id)` | Default implementation over `get_windows`. |
| `get_cursor_position` | **Unsupported**: Sway IPC has no pointer query. Returns `UnsupportedCapability`; `--anchor cursor` fails with the standard message. |
| `WindowControl::transform_window` | One `RUN_COMMAND` (section 4). |
| `set_window_state` | `Restore` and `Maximize`/`Minimize`: **unsupported** in the first version (Sway has `fullscreen` and the scratchpad, which are not the same thing; see open question 6). |
| `focus_window` | `[con_id=N] focus`. `defocus_window`: unsupported. |
| `close_window` | `[con_id=N] kill` (verified to close the fixture). |
| `resolve_id` | The command name, or the explicit class (Sway matches by `app_id`). |
| `protocol_version` / `HelperStatus` | `None`, so `HelperStatus::Ok` (`capability.rs:60`): there is no helper. |
| `Provisioner` | `provisioner()` returns `None`; `spawn-at install` prints "No installation required for sway." (the default, `src/platform/mod.rs`). |
| `Host` | Unchanged (Linux). |
| `UpdateHooks`, `SessionActions` | Unchanged: they are the GNOME ones and do nothing unless `XDG_CURRENT_DESKTOP` contains GNOME (`gnome/deploy.rs:407`). The Sway backend records no session action, so `session::finish()` has nothing to do. |

## 4. What the backend declares, and placement

`Capabilities` (`capability.rs:26`), honestly:

| Field | Value | Why |
| :-- | :-- | :-- |
| `runtime_transform` | true | `floating enable`, `resize set`, `move absolute position` work on a mapped window (verified). |
| `claim_wait` | false | No claim waiter in the first version. |
| `close` | true | `kill` (verified). |
| `reveal_control` | **false** | Sway cannot hold a new window invisible until it is placed; section 5. |
| `needs_helper` | false | Plain IPC. |

Not supported and why: cursor position (no IPC), maximize/minimize (no such state), defocus (no meaning), flicker-free placement (section 5).

**Placement.** The shared engine (`spawn-at-core::geometry`) computes the target rect from the work area it is given. For Sway:

1. Work area: the `rect` of the target output's current workspace (verified to be the usable area, in logical pixels, in global coordinates). Screen area: the output `rect`. `scale` is only reported; no scaling math is needed because Sway's coordinates are already logical.
2. Default output: **Changed in the implementation:** the same as on GNOME, the "primary" monitor, which for Sway is the first active output in the order `GET_OUTPUTS` lists them (decision of the owner, question 4); `--monitor N` picks the Nth in that order, and the shared `resolve_monitor_from_layout` does the choosing. (The design had proposed the output marked `"focused": true`.)
3. Command, **one chained RUN_COMMAND so the criteria carry across commas** (`man 5 sway`): `[con_id=N] floating enable, resize set W H, move absolute position X Y`. Order: size first, then the position, so the position is the final one. `move absolute` is used because the engine's coordinates are global (verified on two outputs).
4. Borders: a floating container's `rect` includes server-side decorations (`border`, `deco_rect`); `window_rect` is the part inside. **Decided (question 3) and verified:** `--size` means the whole container (`rect`), which is what the GNOME extension sizes and what `query windows` reports, so the same command gives the same rectangle on both. `resize set` counts the content only, so after the chained command the driver reads the container back once and, if it differs, asks for the content size reduced by the border and title bar (`rect - window_rect`). Verified with `for_window ... border normal 3`: `--size 600 400` gave a container of exactly 600x400 (content 594x372).
5. A size below what the window allows is raised by Sway (75x50 for the floating fixture window, verified; the GNOME extension reports the same situation as "size raised"). The driver then computes the position again with the size the window really has, so an anchor like bottom-right keeps it inside the area (`--size 30 20` at bottom-right with margin 16 gives 75x50 at 1829,1014, the position `verify_and_report_placement` expects).

## 5. Flicker-free strategy

Honest summary: there is no way over IPC to run commands *before the first frame* that is both reliable and safe. The options:

| Option | How | Race | Verdict |
| :-- | :-- | :-- | :-- |
| A. `for_window` rule before launch | Register `for_window [app_id=..] floating enable, resize set .., move ..` then start the app | Runs inside Sway at map time, so no race. But: over IPC only the first command of the list belongs to the rule (verified), a quoted list applies only `floating enable` (verified), rules cannot be removed, accumulate in the compositor, are global (two spawns of the same app collide), and the `app_id` must be known in advance (spawn-at only knows what the user typed, which may differ). A `pid` criterion would be exact but the pid does not exist yet. | Reject as the placement mechanism. Usable only for `mark`. |
| B. React to `window::new` | Subscribe in `arm`, on the matching `new` event send one chained command | The event is emitted when the view is mapped (observed: the node has `rect` 0x0 then); our command comes from another process, so whether it lands before the first frame is **unverified** and cannot be guaranteed: one frame at the tiled position is possible. | **Recommended.** Keep `reveal_control` false and document that a flash can happen. |
| C. Start, then poll and `[con_id=] move` | `exec`, poll `GET_TREE` | Polling interval plus processing, several frames. | Reject (it is B but slower). |
| D. Start on a hidden workspace/output, then move and show | `workspace` games | Changes the user's layout, needs cleanup, breaks focus. | Reject. |

Recommendation: B, with the subscription established in `arm` so no event is missed (the socket buffers events between `arm` and `post_spawn`). Measure the real flash on a real compositor later (the repo's trace/flash analysis in `crates/spawn-at-verify` is GNOME-extension based and would need a Sway source of frame timing, for example `wlr-screencopy`; **unverified** that it is feasible). Until measured, the docs and `--help` must not promise flicker-free placement on Sway.

## 6. Matching a launched process to its window

`window::new` gives `container.pid` (the pid of the Wayland client, from the socket credentials; **verified** equal to the fixture's pid), `app_id` and `id`. Match in this order, only among windows that are **not** in the snapshot taken in `arm`:

1. `pid == child_pid`.
2. `pid` is a descendant of `child_pid` (`platform::linux::is_process_descendant`, `src/platform/linux/process_tree.rs`; this is what `matches_target` already does, `src/commands/mod.rs:50`): wrapper scripts, `env`, launchers that exec.
3. `app_id` (or the XWayland class) matches the entry's hint with `match_window_app_hint` (`commands/mod.rs:95`): launchers that hand over to a running server (`gnome-terminal` style), where the pid never matches. **Implementation detail:** the hint is matched against the class only (the title is not passed, so a window titled like the command is not taken), an empty or `*` hint never matches by rule 3, and a rule 3 window is taken only after a 300 ms grace in which a pid match may still arrive; the first rule 3 window is kept.
4. Nothing matches within 5 seconds (a constant, `place::WINDOW_WAIT`, the same wait the X11 driver uses; `post_spawn` has no access to `window_timeout`): do **not** move any other window. Print the standard "post-spawn hook failed" warning (`spawn.rs:343`) saying the application was started but not placed, and carry on.

## 7. Selection: never automatic

`LinuxBackend::bootstrap` walks `registry::candidates` (`src/platform/linux/registry.rs:55`, `bootstrap` at `src/platform/linux/mod.rs:101`). Before this backend `SPAWN_AT_BACKEND` was lowercased and compared to `x11` only; any other value, including `sway`, was ignored.

The only behaviour change: `SPAWN_AT_BACKEND=sway` (any case, nothing trimmed, like `x11`) selects `BackendId::Sway`. The rule (`src/platform/linux/registry.rs:59`): if `forced == "sway"`, `candidates` returns `[Sway]`; every other input returns exactly what it returned before. `XDG_CURRENT_DESKTOP=sway` without the variable still gets X11 (leak #11 in DESIGN_PLATFORMS.md stays open, on purpose).

The proof: the existing table test (9 forced values x 14 desktops x D-Bus up or down against a copy of the old selection) is **unchanged** and still passes; the Sway cases are three separate tests that say why they differ from that copy (forced `sway`, `SWAY`, `Sway`, `sWaY` select Sway for every desktop and D-Bus result and never ask for the GNOME connection; ` sway`, `sway `, `swayx` and six more near values agree with the old copy; Sway is never a candidate without the variable). Nine more tests run the built binary against a stand-in socket (`tests/sway_backend_cli_test.rs`).

**Forced but unusable is an error, never a fallback.** If `SPAWN_AT_BACKEND=sway` and `SWAYSOCK` is unset or empty, or the socket is not connectable, or the first reply is not valid IPC, `bootstrap` returns `DriverError::IpcError` (`src/platform/linux/mod.rs:107-112`, with `?`, so nothing after it runs) with a message such as `SPAWN_AT_BACKEND=sway, but $SWAYSOCK is not set. Run spawn-at inside a Sway session, or unset SPAWN_AT_BACKEND.` (and the connect error for a bad socket), which `main` prints as "Driver initialization error: ..." and exits 1 (`src/main.rs`, the `init_backend` match). It does not fall through to X11, unlike the GNOME candidate, which falls back silently when D-Bus is unreachable (`src/platform/linux/mod.rs:113-119`). `I3SOCK` is not consulted (open question 7: no i3 backend).

## 8. IPC: hand-roll, no new dependency

Recommendation: hand-roll with `std::os::unix::net::UnixStream` and `serde_json`, which `spawn-at` already depends on (`Cargo.toml:11`).

- The protocol is a 14-byte header plus JSON (section 2, verified by hand with `socat`). Encoding and decoding are about 60 lines and pure functions, unit-testable with byte vectors recorded from the harness.
- Blocking I/O wrapped in `tokio::task::spawn_blocking`, the pattern the X11 driver uses (`x11/mod.rs:394-402`), so no async runtime coupling.
- Dependency cost of the alternative: crates such as `swayipc`, `swayipc-async` or `i3ipc` exist. **The cost could not be measured here**: the machine has no network and none of them is in the local cargo registry (`ls ~/.cargo/registry/src/*/` has none), so `cargo tree` cannot resolve them. For reference, `spawn-at`'s own normal tree has 145 distinct crates on Linux, and `serde`, `serde_json` and `thiserror` are already in it. I expect an async crate to add an async I/O stack; **unverified**. Measure with `cargo add --dry-run` / `cargo tree` before reconsidering.
- The module is `src/platform/linux/sway/` (Linux only, so the Windows/macOS trees are untouched, `Cargo.toml:21-27`).

## 9. Test plan

1. **Pure unit tests, no sway** (run everywhere CI runs): frame encode/decode against the real bytes of section 2; JSON fixtures recorded from the harness (`tests/fixtures/sway/*.json`: empty tree, tiled window, floating window, two outputs, scale 2 with a bar) mapped to `WindowMetadata`, `MonitorLayout` and work areas; the command builder (`[con_id=N] floating enable, resize set W H, move absolute position X Y`, integer formatting, negative coordinates); the matching rules 1 to 4 including "never a window from the snapshot"; the registry table (section 7).
2. **Fake IPC server tests**: a thread owning a `UnixListener` in a temp dir replays recorded frames; tests `bootstrap` (missing `SWAYSOCK`, bad path, non-IPC reply: clear error, no X11 fallback), `arm` + `post_spawn` with scripted `window::new` events (match by pid, by descendant, by app_id, timeout), `close`, `focus`. No sway needed.
3. **Live tests against the private Sway**, `#[ignore]` unless `SPAWN_AT_SWAY_TESTS=1`, using `scripts/sway-headless.sh` and the fixture: `spawn` with each anchor, size and margin on a 1920x1080 output, on scale 2 with a bar (the work area test of section 4), on two outputs; `query windows`; `close`. Each test first asserts that `SWAYSOCK` starts with `/tmp/spawn-at-sway.` so it can never run against a real session.
4. **CI**: an optional job that installs `sway` on the Ubuntu runner and runs group 3. **Unverified** that the runner's package and a headless Sway work; keep it non-required until it has run.
5. **Flash measurement** (section 5): out of scope for the first version.

## 10. Open questions for the owner (answered; the answers are after the list)

1. Is `reveal_control = false` (a possible one-frame flash) acceptable for the first Sway release, with the docs saying so?
2. Should a Sway session later pick Sway automatically (`XDG_CURRENT_DESKTOP=sway`)? This design says no for now; it would change the backend that Sway users get today (X11 through XWayland).
3. Does `--size` mean the container (`rect`, with borders and title bar) or the content (`window_rect`)? The GNOME driver sizes the content; I suggest the content, which means adding the border widths from `rect - window_rect`.
4. Default output when `--monitor` is not given: the focused workspace's output. OK?
5. May CI install `sway` from the Ubuntu archive (section 9, item 4)?
6. Do you want maximize/minimize mapped (to `fullscreen` and the scratchpad), or left unsupported?
7. `I3SOCK`/i3: Sway and i3 share the protocol; should `SPAWN_AT_BACKEND=i3` also exist (and read `I3SOCK`), or stay out of scope?

**Answers (owner, applied in the implementation):** (1) a possible one-frame flash is acceptable for a first release; Sway is documented as EXPERIMENTAL and `reveal_control` stays false. (2) Never auto-select Sway; `candidates()` is unchanged for every existing environment and only `SPAWN_AT_BACKEND=sway` selects it. (3) `--size` means what it means on GNOME (the whole window frame), so the same command gives the same result; the placement math of `spawn-at-core` is reused unchanged. (4) The default monitor is the same as on GNOME ("primary"; first output for Sway). (5) No CI job installs sway in this change. (6) No maximize/minimize mapping in v1; declared unsupported. (7) No i3 backend.

## 11. The harness

`scripts/sway-headless.sh` (`start` / `env` / `status` / `stop`, `--dry-run`, `--extra-config FILE`):

- Starts `sway` with `WLR_BACKENDS=headless`, `WLR_LIBINPUT_NO_DEVICES=1` and `WLR_RENDERER=pixman` (added so no GPU is touched), a config with one `HEADLESS-1` 1920x1080 output and no bar and no XWayland, in a private `0700` temp directory that is also the sway process's own `XDG_RUNTIME_DIR`. Its Wayland and IPC sockets are created there and nowhere else; `WAYLAND_DISPLAY`, `DISPLAY`, `SWAYSOCK`, `I3SOCK` are removed from its environment, so it can never nest into another compositor.
- Environment is set only on the child, or printed (`export SWAYSOCK=...`, `export WAYLAND_DISPLAY=<absolute path>`, `unset DISPLAY`) for the caller's subshell. Nothing goes to `systemctl --user set-environment`, `~/.profile` or any file outside the private directory.
- Records the PID in `<dir>/sway.pid`. `stop` kills only that PID, after checking `/proc/PID` is a `sway` whose `XDG_RUNTIME_DIR` is that directory and whose `WLR_BACKENDS` is `headless`; otherwise it refuses and removes nothing. It then removes the directory (which must carry the marker file, be owned by you, match `*/spawn-at-sway.*` and not be the real runtime dir).
- `--dry-run` prints what `start` or `stop` would do and does nothing.
- Checks run on this machine: a recorded PID that was a `sleep` was refused and left alive; `stop --dir /tmp` was refused; a normal `stop` left no sway, swaybar or fixture process and no directory; `gnome-shell` kept its PID and command line; no journal line mentioned spawn-at, sway or an assertion during the runs.

To try it yourself (not run by the author in your session):

```bash
scripts/sway-headless.sh start --dry-run
eval "$(scripts/sway-headless.sh start)"            # prints the exports; use a subshell or a spare terminal
swaymsg -t get_outputs                              # talks to the private sway only
WAYLAND_DISPLAY="$WAYLAND_DISPLAY" target/debug/fixture --protocol wayland --title demo --exit-after-ms 60000 &
swaymsg -t get_tree
scripts/sway-headless.sh stop --dir "$SPAWN_AT_SWAY_DIR"
```

## 12. Implementation status

Built on `feat/sway-backend` in five commits (codec and types; read-only driver; placement; registry; script and docs). No new dependency: `Cargo.toml` and `Cargo.lock` are unchanged, the IPC is `std::os::unix::net::UnixStream` plus the `serde_json` already in the tree. The module exists only on Linux (`platform/linux/sway/`), so the three non-Linux `cargo check` runs and the CI job `check-other-os` do not see it.

| File (`src/platform/linux/sway/`) | Role |
| :-- | :-- |
| `ipc.rs` | The 14-byte frame: encode, decode, limits, errors. Pure. |
| `types.rs` | Serde types for outputs, workspaces, the tree and `window` events; conversion to `WindowMetadata` and `MonitorLayout`. Pure. |
| `command.rs` | The `RUN_COMMAND` strings (integers only; nothing the user typed reaches Sway's command line). Pure. |
| `matching.rs` | Pid, descendant and app id rules with the grace and the snapshot. Pure. |
| `client.rs` | Blocking connection with timeouts; the `$SWAYSOCK` messages. |
| `place.rs` | Subscribe and snapshot, wait for the window, place it, correct the frame, single-window commands. |
| `mod.rs` | `SwayDriver` (`Driver` and `CompositorBackend`): one short connection per call, the only state is the subscription `arm` leaves for `post_spawn`. |
| `fake.rs`, `tests_write.rs` | Test only: a scripted IPC server and the driver tests against it. |

What differs from sections 1 to 11:

- Primary monitor and the default monitor: first output, as on GNOME (section 4, answer 4).
- `--size` is the whole container; the correction after `resize set` and the re-anchoring of a raised window are additions (section 4, items 4 and 5).
- `state` in `query windows` is `normal` or `fullscreen`, the GNOME words, not `floating` and `tiled`.
- A window in the scratchpad (`__i3_scratch`) is listed with `minimized` true, no monitor and no workspace.
- No `window_timeout`: `post_spawn` waits a fixed 5 s (section 6). `--no-wait` still waits for the placement, because the pipeline awaits `post_spawn` in every mode (`spawn.rs:343`).
- `FocusIntent::Exclusive` adds `focus`; `Leave` cannot be honoured because Sway focuses a new window by its own rules.
- Only the first entry of a batch is placed (the X11 driver does the same).
- Maximize, minimize, restore and defocus return the standard "does not support this feature" error; `--anchor cursor` and `--monitor cursor` use (0, 0) because the pointer cannot be read.

What was run (all on the private headless Sway 1.9 of `scripts/sway-headless.sh`, with the repo's fixture window as the only client; the GNOME session was not touched and `gnome-shell` kept its PID):

- `scripts/sway-live.sh`: 62 checks, 0 failed (list in [TEST_PLAN.md](TEST_PLAN.md) section 7).
- By hand while building: the same placements through `spawn-at spawn --json`, `close`, `transform`; a real border and title bar; `--no-wait`; forced Sway with `SWAYSOCK` unset.

Not verified (and not claimed): a real Sway session or real applications; XWayland windows (disabled in the harness); Sway versions other than 1.9; i3; frame timing before the first paint (the one-frame flash of section 5 has not been measured); `--area screen` and `--area unbounded`; clients that map at a size other than the one asked; scales other than 1 and 2; more than two outputs.
