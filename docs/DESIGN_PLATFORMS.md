# Design: restructuring spawn-at for more platforms

Status: proposal, written from the tree at `8822484` (v0.2.0-beta.3 merged). The document itself changes no code; migration step 1 (section 6) was then done in this same branch and is marked there. Citations below were re-checked after that step. Every claim about the code cites `file:line`; anything that could not be confirmed here is marked **unverified**. Statements about other platforms come from public knowledge, not from running anything, and are marked the same way.

Goal: a structure that lets spawn-at support GNOME Wayland (today), other Wayland compositors, X11, Windows, macOS and more, **without changing the working GNOME Wayland behavior**.

Contents: 1 Inventory, 2 Existing abstraction, 3 Leaks, 4 Capability model, 5 Feasibility per target, 6 Migration plan, 7 Risks and open questions, 8 Findings worth remembering.

## 1. Inventory

Line counts are `wc -l` of the tracked files (tests included, so a file's production part is smaller; where I know the first test line I say so). Labels: **neutral** (no platform assumption), **GNOME** (GNOME Shell only), **Linux** (Linux but not GNOME), **mixed**.

### Binary crate `spawn-at` (`src/`, 36 files, 19,409 lines)

| Path | Lines | Responsibility | Label |
| :-- | --: | :-- | :-- |
| `src/main.rs` | 455 | Entry point; dispatches subcommands; the whole `install` and `uninstall` orchestration (existing-install check, scope prompts, config write, autostart, session end) | mixed: orchestration is neutral, but `~/.local/bin` and `/usr/local/bin` are hard-coded at `main.rs:131-133` |
| `src/cli/args.rs` | 673 | clap argument definitions | neutral (one help text names Wayland App ID / WM_CLASS, `args.rs:236`) |
| `src/cli/mod.rs`, `src/cli/tests.rs` | 84, 1,338 | CLI root, CLI tests | neutral |
| `src/commands/mod.rs` | 1,487 | Window matching (`matches_target` :48, `match_window_app_hint` :93), wait for a spawned window (`wait_for_spawn` :189, `wait_for_state_change` :301, `settle_window_geometry` :353), shared test doubles | mixed: neutral logic, but the process-ancestry check is `cfg(linux)` (:75-80, :218, :254) |
| `src/commands/spawn.rs` | 1,342 (production ends at :618) | `run_spawn` / `run_spawn_with` (:47, :58-588, one 530-line function), `configure_child_process` (:588) | mixed: pipeline is neutral, config store is Linux (:52), `setsid` is unix (:605) |
| `src/commands/query.rs`, `transform.rs`, `lifecycle.rs`, `focus.rs`, `config.rs` | 817, 372, 209, 122, 40 | Query / transform / close-maximize-minimize / focus / `config` subcommands over the backend trait | neutral, except `config.rs:5` (Linux store) and the extension-protocol messages (see section 3) |
| `src/config.rs` | 430 | `Config` (update settings) file I/O, **login autostart entry** (:186-263, an XDG `.desktop` file) | mixed |
| `src/update.rs` | 1,424 (production ends at :931) | Release check against GitHub (curl), update banner, `perform_upgrade` (:589), `run_update` (:751), menu | mixed: version logic neutral; asset name, curl/tar, GNOME deploy are Linux (:590, :620, :634, :660) |
| `src/diagnostics.rs` | 1,481 (production ends at :613) | Typed diagnostics and rendering, colour capability | neutral |
| `src/target.rs` | 592 (production ends at :262) | `WindowSelector`, `WindowMetadata` (:69), target resolution | neutral |
| `src/session.rs` | 856 (production ends at :362) | End of run: the log-out/reload question, advice, the action | mixed: the machine is neutral, the texts and the real ops are GNOME (`session.rs:279-291`, `:352-360`) |
| `src/ui.rs`, `src/prompts.rs` | 308, 207 | Output layout, colour; every dialoguer prompt | neutral (prompts name `~/.local/bin`, `prompts.rs:46-69`) |
| `src/platform/mod.rs` | 336 | The backend trait `CompositorBackend` (:124), `ClaimSubscription` (:57), `ClaimResult` (:40), `WindowState` (:64), install/uninstall args (:82, :110), `init_backend` (:328) | neutral by intent; install args embed GNOME flags (:104, :117) |
| `src/platform/installer.rs`, `escalate.rs`, `install_notice.rs`, `upgrade.rs` | 235, 93, 108, 759 | Binary install/uninstall into `~/.local/bin` or `/usr/local/bin`; privilege elevation (`sudo` for Linux and macOS, stub for Windows, `escalate.rs:11-55`); untested-platform notice (`gnome_notice`, `install_notice.rs:13`); existing-install detection and clean install | mixed: Linux paths and `cfg!(linux)` guards (`installer.rs:77`, `:166`), unix-only `PermissionsExt` at `upgrade.rs:68` (unguarded) |
| `src/platform/linux/mod.rs` | 651 (production ends at :235) | `LinuxBackend` (:63), backend choice (:73-98), X11 monitor/cursor helpers (:17-60), the process-ancestry re-export (step 2: the code is in `process_tree.rs`) | Linux (backend choice is mixed GNOME/X11) |
| `src/platform/linux/config_store.rs` | 141 | `SettingsStore` for the config file (`XDG_CONFIG_HOME`, `HOME`) | Linux |
| `src/platform/linux/xdg.rs` | 198 | `resolve_linux_app_id` (:30): desktop-file search for an app id | Linux |
| `src/platform/linux/gnome/mod.rs` | 1,886 (production ends at :1123) | The GNOME Wayland driver: install/uninstall args (:75-136), session detection (`loginctl`, :261-340), the extension files embedded (:147-236), logout/reload helpers (:349, :376), D-Bus driver (:417-1120) | GNOME |
| `src/platform/linux/gnome/mechanics.rs` | 640 | The instruction language sent to the extension (`Instruction`, :42; `build_instructions_for_entry`, :171) | GNOME |
| `src/platform/linux/gnome/logout.rs` | 411 | "Is a log-out needed?" decision from file, `gnome-extensions info` and `/proc` facts | GNOME |
| `src/platform/linux/gnome/dbus.rs` | 51 | zbus proxy for `org.gnome.Shell.Extensions.SpawnAt` (:3-6) | GNOME |
| `src/platform/linux/x11/{mod,session,interception,atoms,process,tests}.rs` | 395, 643, 294, 37, 30, 265 | The X11 driver: x11rb connection, EWMH operations, spawn interception loop | Linux (X11; also runs under XWayland) |
| `build.rs` | 119 | Version from `git describe` | neutral |

### Library crate `spawn-at-core` (`crates/spawn-at-core`, 11 files, 4,081 lines): **neutral**

`driver.rs` (140: `Driver` trait :132, `Batch`, `Entry`, `Armed`, `DriverError`), `geometry.rs` (1,680: placement math), `ready_wait.rs` (535: the "has the window answered a resize" state machine; no I/O, no clock), `settings.rs` (584, tests in `settings/tests.rs` 365: config keys and the `SettingsStore` trait :333), `wait_override.rs` (203), `resize_report.rs` (147), `lib.rs` (17). Dependencies: serde, toml, async-trait, optional clap (`Cargo.toml`). I found no platform-specific code in it; the only XDG mention is the pure config-location rule (`settings.rs:361-370`).

### Other crates and assets

| Path | Size | Responsibility | Label |
| :-- | --: | :-- | :-- |
| `crates/spawn-at-verify` | 147 files, 32,484 lines | Developer verification harness: runs spawn-at against a fixture window, reads logs and traces, TUI | mixed: runner, oracle, report are neutral; `log_source.rs:39` (`journalctl`), `ext_logging.rs:20` (`gdbus`), `display.rs:77-85` (`xrandr`, `gdbus GetCurrentState`), `probe.rs:150` (`gnome-shell --version`) are GNOME/Linux. Depends only on core. |
| `crates/xtask` | 7 files, 952 lines | Repository tooling | neutral |
| `assets/gnome/modern`, `assets/gnome/legacy` | 11 and 9 files, 3,661 and 2,793 lines | The GNOME Shell extensions (JavaScript), embedded in the binary; `extension.js` 1,799 lines; a JS port of `ready_wait` | GNOME |
| `tests/` | 35 files, 5,644 lines | Node tests of the extension (183), Rust CLI tests, ready-wait vectors (`tests/ready_wait_vectors`) | GNOME (node), neutral (vectors) |
| `verify/` | 62 files | Test specs (TOML), golden placement | neutral specs, GNOME runner |
| `install.sh`, `scripts/`, `.github/workflows/` | | Bootstrap installer (Linux x86_64 only, `install.sh:27,42`); CI and release on `ubuntu-24.04` only (`ci.yml:20`) | Linux |

## 2. What abstraction already exists

1. **`Driver` (core, `driver.rs:132`)**: `arm(Batch) -> Armed` and `disarm`. A declarative `Batch` of `Entry` values (app hint, `PlacementParams`, wait override) with reveal / focus / urgency policy (`driver.rs:12-72`). `Armed` carries environment variables to inject into the child (`driver.rs:73`). This is the platform-neutral heart and it is used as intended: `run_spawn_with` builds a batch (`spawn.rs:291`) and never sees D-Bus or X11.
2. **`CompositorBackend` (binary crate, `platform/mod.rs:124`)**: extends `Driver` with setup and teardown (`install_notice` :132, `install` :137, `uninstall` :143), capability flags (`supports_runtime_transform` :151, `supports_claim_wait` :159, `supports_close` :164), claim waiting (`prepare_claim_wait` :169), queries (`resolve_id` :195, `get_cursor_position` :198, `get_monitors` :203, `get_workareas` :208, `get_windows` :213, `get_layout` :218, `protocol_version` :241) and actions (`transform_window` :248, `move_window` :262, `move_resize_window` :269, `set_window_state` :281, `focus_window` :290, `defocus_window` :295, `close_window` :305, `post_spawn` :312). Most methods have an "unsupported" default, so a new backend can start small.
3. **`ClaimSubscription` (`platform/mod.rs:57`)**: "the compositor tells me when the armed window was placed", with `ClaimResult` (:40).
4. **`SettingsStore` (core, `settings.rs:333`)**: where the config text lives; implemented by `LinuxConfigStore` (`config_store.rs:44`). This is the one place where a neutral trait + one platform implementation is already the right shape.
5. **cfg gates**: `pub mod linux` and `NativeBackend` only on Linux (`platform/mod.rs:16-25`; off Linux `platform::linux` is the compile-only stub `non_linux.rs`, step 1); `init_backend` returns "Unsupported operating system" elsewhere (`mod.rs:327-334`; `main` now stops earlier through `ensure_supported_os`, `mod.rs:319`); `escalate.rs` has per-OS branches for Linux, macOS, Windows (:11-55, :64, :82); a few `cfg(unix)` blocks (`spawn.rs:605`, `update.rs:303,367,611,696`, `installer.rs:118`).
6. **`LinuxBackend` (`linux/mod.rs:63`)** is a thin forwarder that picks GNOME or X11 (`bootstrap`, :73-98) and delegates every method (:109-234).

**How well it is respected.** The command layer (`commands/*`) talks only to `CompositorBackend`, and a GNOME-only module (`gnome::mechanics`) is not used outside the GNOME driver in production code (its two other references are in tests: `spawn.rs:884,901` are after the test module starts at :619; `x11/tests.rs:115`). That is a good result. The trait is, however, GNOME-shaped in four ways: (a) setup/teardown and GNOME flags live inside it (`InstallArgs.gnome` :98, `UninstallArgs.gnome` :111); (b) the "claim" model (`prepare_claim_wait`, `SpawnClaimed` signal) mirrors the GNOME extension; (c) `protocol_version` is the GNOME extension's protocol number (`gnome/mod.rs:461-464`), compared against constants in neutral commands (section 3); (d) the `supports_*` flags are barely used: only `supports_runtime_transform` (`transform.rs:18`) and `supports_close` (`lifecycle.rs:34`) are read in production; `supports_claim_wait` carries an `#[allow(dead_code)]` (`mod.rs:158`). Capability checks therefore mostly happen by calling a method and matching `UnsupportedCapability`.

## 3. Where platform assumptions leak into neutral code

| # | Location | Assumption | What a neutral version needs |
| :-- | :-- | :-- | :-- |
| 1 (fixed in step 1) | `config.rs:108`, `config.rs:151`, `commands/spawn.rs:52`, `commands/config.rs:5` | Unconditional use of `crate::platform::linux::config_store::LinuxConfigStore`, but `platform::linux` exists only on Linux (`platform/mod.rs:16`). **By reading, the crate cannot compile on macOS or Windows today** (unverified: no cross target is installed here, so I could not run `cargo check --target ...`). | A `Host::config_store()` (or `platform::config_store()`) that returns the right `SettingsStore` per OS. |
| 2 (fixed in step 1) | `platform/upgrade.rs:68` (and `:365` in tests) | `use std::os::unix::fs::PermissionsExt` unguarded; executable-bit test | `Host::is_executable(path)` with a Windows rule (extension / `PATHEXT`). |
| 3 | `commands/mod.rs:75-80`, `:218`, `:254`; `linux/process_tree.rs:4-33` (step 2: one copy; it was `linux/mod.rs:59-85` and `x11/process.rs:4-30`) | Process ancestry through `/proc/<pid>/stat`, duplicated in two places (before step 2), gated to Linux only in the callers | A `ProcessTree` capability (`parent_of(pid)`), per-OS implementation (`/proc`, `sysctl`/`proc_pidinfo` on macOS, ToolHelp snapshot on Windows), one shared `is_descendant`. |
| 4 | `main.rs:134-136`, `installer.rs:39-44`, `prompts.rs:46-69` | Binary install directories `~/.local/bin` and `/usr/local/bin`; `HOME` | `Host::bin_dirs(scope)` (Windows: `%LOCALAPPDATA%\Programs`, `%ProgramFiles%`, as already sketched in `installer.rs:24-27`); prompts built from the returned paths. |
| 5 | `config.rs:186-263` | Login autostart = XDG `.desktop` file with `X-GNOME-Autostart-enabled` | `Host::autostart` trait: XDG on Linux, LaunchAgent plist on macOS, Run key or Startup folder on Windows. |
| 6 | `update.rs:590`, `:620`, `:634` | Release asset `spawn-at-x86_64-unknown-linux-gnu.tar.gz`; `curl` and `tar` as external programs | Asset name from the target triple; an HTTP client and archive reader in-process (or per-OS commands). Windows needs `.zip`. |
| 7 | `update.rs:660-672`, `:721-728` | `XDG_CURRENT_DESKTOP` contains GNOME then "deploy the GNOME extension from the new binary" and ask about log-out | A `Backend::post_upgrade()` hook and `Backend::session_hint()`; no desktop check in update code. |
| 8 | `session.rs:279-291`, `:352-360`, `prompts.rs:83-95` | Texts name `--gnome-wayland-logout`, GNOME Shell; real ops call `gnome::log_out_wayland` / `restart_gnome_shell_x11` | `SessionAction` supplied by the backend (log out, reload shell, nothing) with its own wording. The state machine can stay. |
| 9 | `commands/spawn.rs:101`, `commands/transform.rs:44`, `commands/query.rs:74` | Neutral commands compare `protocol_version()` with 4 / 5 (and `gnome/mod.rs:461-464` has 3 and 6) and print "extension is older ... run `spawn-at install` and log out" | A backend-level `check_helper()` returning a typed `HelperStatus { Ok, Outdated { fix: String } }` per feature needed. |
| 10 | `platform/mod.rs:104`, `:117` | GNOME flags (`--gnome-auto-enable`, `--gnome-wayland-logout`, ...) are fields of the neutral install args | Backend-specific args parsed separately (a flattened per-backend struct selected at run time, or generic `--backend-opt key=value`); `--help` output must stay identical for GNOME. |
| 11 | `platform/linux/mod.rs:73-98` | Backend choice: `SPAWN_AT_BACKEND=x11` forces X11; else `XDG_CURRENT_DESKTOP` contains GNOME and D-Bus connects; **everything else becomes the X11 driver**, including Sway, Hyprland and KDE Wayland sessions, where it sees XWayland windows only. | A registry of backends with `detect() -> Option<Confidence>` per backend, and an explicit "no usable backend" result instead of a silent X11 fallback. |
| 12 | `platform/mod.rs:327-334` | Non-Linux returns "Unsupported operating system" | A `NullBackend` for unsupported hosts so `--help`, `config`, `update` still work. |
| 13 | `commands/spawn.rs:304-313`, `:605-615` | Launch = `std::process::Command` + environment from `Armed`; child detached with `setsid` (unix only) | `Host::launch(cmd, env)` hiding `CREATE_NEW_PROCESS_GROUP` (Windows) and `posix_spawn` details. Environment tokens (`XDG_ACTIVATION_TOKEN`, `DESKTOP_STARTUP_ID`, `gnome/mod.rs:601-602`) are a Wayland/X11 convention; other OSes ignore them. |
| 14 | `platform/linux/xdg.rs:30`, `gnome/mod.rs:689-692` | `resolve_id` = desktop-file lookup (Linux app ids) | `Backend::resolve_app_id(command)`: bundle id (macOS), AppUserModelID / exe name (Windows). |
| 15 | `target.rs:69-99`, `commands/mod.rs:93-170` | `WindowMetadata.class` / `app_id` matching tuned for WM_CLASS vs reverse-DNS app ids (`gnome-terminal` -> `gnome-terminal-server`) | Keep the matcher; add a documented mapping for what `class` and `app_id` mean per backend. |
| 16 | `build.rs`, `install.sh:27,42`, `.github/workflows/*.yml` | Release/CI assume Linux x86_64 on `ubuntu-24.04` | Matrix over targets; only `cargo check` on other OSes at first. |
| 17 | `update.rs:367-380` | `libc::ioctl(TIOCGWINSZ)` for terminal width (guarded by `cfg(unix)` at :367) | Fine today; Windows needs a console API or a crate. |
| 18 | `Cargo.toml` | `zbus`, `x11rb`, `libc` are unconditional dependencies; no `[target.'cfg(...)'.dependencies]` | Per-target dependency tables so Windows/macOS builds do not need them (whether they would even compile there is **unverified**). |

Top five by impact: **#1** (the crate does not compile off Linux), **#11** (silent X11 fallback for every non-GNOME desktop), **#10** (GNOME flags in neutral install args), **#9** (extension protocol numbers in neutral commands), **#5/#4** (autostart and install directories hard-coded to XDG/`~/.local`).

## 4. Proposed capability model

A backend is a bundle of small capabilities. A backend implements only what its platform can do and states so in a `Capabilities` value (replacing today's three `supports_*` booleans and the "call it and see" pattern).

| Capability | Operations | Implemented today by |
| :-- | :-- | :-- |
| **Arm** (declare intent before launch) | `arm(Batch)`, `disarm` | `Driver` (`driver.rs:132`); GNOME `gnome/mod.rs:559,613` (D-Bus `ArmSpawn`); X11 `x11/mod.rs:39,54` (starts the interception loop) |
| **Launch** | start the app with the armed environment; detach it | `spawn.rs:304-313`, `configure_child_process` :588 (neutral, unix-only detach :605) |
| **Discover** (windows, monitors, work areas, cursor) | `get_windows`, `get_monitors`, `get_workareas`, `get_layout`, `get_cursor_position` | GNOME `gnome/mod.rs:1004,931,994,948,921`; X11 `x11/mod.rs:119,99,109,129,79` |
| **Identify** (map a command to an app id; match windows to a process) | `resolve_id`, process ancestry | `xdg.rs:30` via `gnome/mod.rs:689`; `commands/mod.rs:48-90` and `/proc` helpers (leaks #3, #14) |
| **Place** (move/resize) | `transform_window`, `move_window`, `move_resize_window` | GNOME `gnome/mod.rs:881,1014`; X11 `x11/mod.rs:139,158,170` |
| **Ready signal** (the window has settled) | claim subscription; ready-wait; polling fallback | GNOME `prepare_claim_wait` `gnome/mod.rs:643`, `ClaimSubscription` :471-556, state machine in core `ready_wait.rs` (also ported to `assets/gnome/modern/ready_wait.js`); generic polling `wait_for_spawn` `commands/mod.rs:189`, `settle_window_geometry` :353; X11 loop `x11/interception.rs` |
| **Reveal control** (hold a window invisible until placed: "cloak") | part of the `Instruction` list | GNOME only (`mechanics.rs:42`, `assets/gnome/modern/cloak.js`); X11 has no equivalent (windows can flash, per README) |
| **State / focus / close** | `set_window_state`, `focus_window`, `defocus_window`, `close_window` | GNOME `gnome/mod.rs:1021,1047,1063,1084`; X11 `x11/mod.rs:189,283,295,355` |
| **Helper provisioning** (install/remove what the compositor needs) | deploy extension, enable it, report whether it runs and is current | GNOME `install`/`uninstall` `gnome/mod.rs:705,822`, facts in `logout.rs`; X11 and others: nothing needed |
| **Session action** | log out / reload shell, with advice | `session.rs` + `gnome::log_out_wayland` (:349), `restart_gnome_shell_x11` (:376) |
| **Host services** (not backend-specific; per OS) | config dir (`SettingsStore`), data dir, autostart entry, binary install dirs, privilege elevation, process tree, executable test, terminal size | Linux only: `config_store.rs`, `config.rs:186-263`, `installer.rs:39`, `escalate.rs`, `linux/process_tree.rs`, `update.rs:367` |
| **Release channel** | which asset to download, how to unpack, how to replace the running binary | `update.rs:590-736` (Linux tar.gz only) |

### Trait boundaries

* **Keep** `Driver` in `spawn-at-core` unchanged (arm/disarm + `Batch`).
* **Split `CompositorBackend`** into narrow traits, keeping `CompositorBackend` as a supertrait bundle so call sites and test doubles do not change on day one:
  * `WindowQuery` (discover, identify),
  * `WindowControl` (place, state, focus, close),
  * `ReadySignal` (`prepare_claim_wait`, `ClaimSubscription`; absent means "poll"),
  * `Provisioner` (install, uninstall, notice, `check_helper`, `post_upgrade`) with **typed inputs and results** instead of `println!` plus GNOME-specific args,
  * `Capabilities` (a plain struct returned by the backend: `place_before_map`, `reveal_control`, `ready_signal`, `close`, `focus`, `cursor`, `needs_helper`).
* **Add a `Host` trait** (or a set of free functions behind `cfg`) for the per-OS services in the table above; the Linux implementation is what exists today, moved, with identical results.
* **Backend registry**: `fn detect(env) -> Vec<(BackendId, Confidence)>`; `init_backend` picks the best or reports "no supported backend for this session" with what was detected. `SPAWN_AT_BACKEND` keeps working.

### Crate / module layout

Start inside the binary crate (cheap, reversible), split into crates only when dependency weight demands it:

```
spawn-at-core            pure: geometry, ready_wait, settings, Driver/Batch   (unchanged; may gain
                         WindowMetadata, MonitorLayout, Capabilities, HelperStatus)
src/
  cli/ commands/ config.rs update.rs diagnostics.rs target.rs ui.rs prompts.rs   shared, neutral
  backend/                 traits: WindowQuery, WindowControl, ReadySignal, Provisioner, Capabilities
                           registry.rs: detect + init_backend
  backends/
    gnome_wayland/         today's platform/linux/gnome/* (moved, split into files)
    x11/                   today's platform/linux/x11/*
    sway/ hyprland/ kwin/  later
    win32/ macos/          later
  host/
    mod.rs                 Host trait
    linux.rs  macos.rs  windows.rs
```

What stays shared: the CLI and all subcommands, config and settings, policies (ready-wait strategy, wait override, geometry), diagnostics, the update logic, the prompts/layout/session machinery, `spawn-at-verify` specs and fixture, `xtask`. What becomes per backend: arm/transform/discover, helper provisioning, session wording. What becomes per OS: the host services.

## 5. Feasibility per target

All of this is from public knowledge and **unverified** unless a line says otherwise; none of it was run here.

### Other Wayland compositors

Wayland has no standard protocol for one client to move another client's window, so each compositor needs its own route. A generic wlroots protocol (`wlr-foreign-toplevel-management`) lists, activates, closes, minimizes and maximizes toplevels but (as far as I know) does not set geometry (unverified). Clients also cannot position themselves (`xdg-shell`).

* **Sway / i3 (and other i3-IPC compositors).** Native: a unix socket (`$SWAYSOCK`) with a JSON IPC; subscribe to `window` events, query the tree, send commands such as `[app_id="x"] floating enable, move position X Y, resize set W H`. No helper to install. Limits: tiled windows follow the layout, so spawn-at must float them; the window may be mapped (and visible) before the command lands, so a flash is possible; there is no "cloak". Pre-placement is possible with `for_window` rules registered through IPC. CI: headless sway (`WLR_BACKENDS=headless`) is commonly used in CI; plausible, **unverified** here.
* **Hyprland.** Native: a command socket and an event socket under `$XDG_RUNTIME_DIR/hypr/<instance>/`; `hyprctl dispatch` equivalents (`movewindowpixel`, `resizewindowpixel`), and window rules including `exec [float; move X Y; size W H] command`, which is very close to spawn-at's "arm, then launch" model. Events such as `openwindow` give a ready signal. No helper. CI: running Hyprland headless is less common (**unverified**); testing likely needs a real machine or a VM with GPU emulation.
* **KDE Plasma (KWin).** Native route is a KWin script (JavaScript) loaded over D-Bus (`org.kde.KWin` scripting interface), reacting to `workspace.windowAdded` and setting `frameGeometry`; KWin window rules can also pre-place windows. This is the GNOME-extension shape again: a **helper** (script) to deploy, enable and version. Limits: Plasma 5 vs 6 API differences (**unverified**). CI: needs a real session or a heavy VM.
* **wlroots limits in general**: no cloak; ready detection from compositor events when present, else polling `get_windows`.
* **Impossible/restricted**: GNOME-style cloaking and a pre-map claim (this is what the extension buys) have no equivalent on most compositors; promise placement, not flicker-free placement.

### X11

Already implemented (`platform/linux/x11/*`), using EWMH (`_NET_CLIENT_LIST`, `_NET_WM_PID`, `_NET_WORKAREA`, `_NET_MOVERESIZE_WINDOW`, `_NET_WM_STATE`) and XRandR monitors via x11rb (`linux/mod.rs:17-60`, `x11/session.rs:7-9`). Native; no helper. Limits: placement happens after map, so windows can flash (README, `x11/interception.rs` loop); needs a running EWMH window manager. CI: Xvfb plus a lightweight window manager is a standard setup; plausible, **unverified** here. Under Wayland it only sees XWayland clients (leak #11).

### Windows

Native Win32: enumerate with `EnumWindows` / `GetWindowThreadProcessId` (pid match), place with `SetWindowPos`, state with `ShowWindow`, monitors and work areas with `GetMonitorInfo`, a creation signal with `SetWinEventHook` (`EVENT_OBJECT_SHOW` / create). No helper process is needed. Limits: **no reliable pre-map placement** for arbitrary apps (the hook fires after creation, so a flash is likely); `SetForegroundWindow` is restricted by the foreground lock; a non-elevated spawn-at cannot move elevated windows (UIPI); DPI virtualization affects coordinates. DWM cloaking exists (`DWMWA_CLOAK`) but whether a third party can set it on another process's window is **unverified**. Rust: the `windows` crate. CI: GitHub `windows-latest` runners exist, so `cargo check`/unit tests are cheap; whether a GUI window test works on the runner's desktop session is **unverified**.

### macOS

Native: the Accessibility API (`AXUIElement`, set `kAXPositionAttribute` / `kAXSizeAttribute`), `AXObserver` for window-created notifications, `NSWorkspace` for app launches, `CGWindowListCopyWindowInfo` for listing. **The user must grant Accessibility permission** (System Settings, Privacy & Security); the process can trigger the prompt (`AXIsProcessTrustedWithOptions`) but cannot grant it. Limits: Spaces and Stage Manager affect geometry; no pre-map placement; some listing details may need Screen Recording permission on recent macOS (**unverified**). Rust: `objc2`, `core-graphics`, accessibility bindings. CI: `macos-latest` runners can build and unit test; granting Accessibility to a CI process is not practical, so live tests need a real machine (**unverified** on exact runner behavior).

### Summary

| Target | Native? | Helper? | Pre-map / flicker-free | CI-testable |
| :-- | :-- | :-- | :-- | :-- |
| GNOME Wayland (today) | extension over D-Bus | yes (extension) | yes (cloak + claim) | node and Rust unit; live suites on one real machine |
| Sway / i3 | IPC socket | no | no (flash possible) | likely headless (unverified) |
| Hyprland | IPC sockets, `exec` rules | no | partly (rules before launch) | hard (unverified) |
| KDE KWin | KWin script | yes (script) | probably no (unverified) | hard |
| X11 | EWMH + XRandR | no | no (flash) | Xvfb + WM (unverified) |
| Windows | Win32 | no | no | build/unit yes; live unverified |
| macOS | Accessibility API | no (permission) | no | build/unit yes; live needs a real machine |

## 6. Migration plan

Rule for every step: **behavior-preserving, one logical change, no new feature**, proven in the beta.2 style: sorted test names per binary unchanged, `cargo test` counts unchanged, `--help` of `spawn-at` and each subcommand byte-identical (`install --help` included), `git diff -M --stat` for moves, sha256 of goldens (`verify/golden/placement.json`, `tests/ready_wait_vectors`) unchanged, the node suite (183) unchanged, and the GNOME live suite re-run by the owner on GNOME 46 Wayland before the release that contains the step.

| # | Change | Proof | Size | Risk |
| :-- | :-- | :-- | :-- | :-- |
| 0 **(done)** | Baseline snapshot (script in `scratch/baseline_platform/`, described in [REFACTOR_PROOFS.md](REFACTOR_PROOFS.md)): sorted test names, help outputs, golden hashes, installer fake-curl matrix | n/a (creates the proof) | S | none |
| 1 **(done)** | Make the crate compile off Linux (leaks #1, #2): a compile-only stub `src/platform/non_linux.rs` mounted as `platform::linux` on other OSes (the real `config_store.rs` reused, `is_wayland_session() -> false`), `is_executable_file` split by `cfg(unix)`, and `ensure_supported_os()` that prints "spawn-at does not support this operating system yet" and exits 1. Neutral files unchanged. `cargo check --locked -p spawn-at` passes for `x86_64-pc-windows-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`; CI job `check-other-os`. (The tests and `spawn-at-verify` do not compile off Linux yet.) | tests unchanged; help diff empty | S | low |
| 2 **(done)** | One `ProcessTree` module (merge the two `/proc` copies, leak #3), callers unchanged: `x11/process.rs` moved to `linux/process_tree.rs`, the duplicate in `linux/mod.rs` removed, `platform::linux::is_process_descendant` kept as a re-export | sorted test names; the existing process tests keep passing | S | low |
| 3 | Pure-move split of `gnome/mod.rs` (1,886 lines) into `args.rs`, `detect.rs`, `deploy.rs` (install/uninstall), `driver.rs`, `tests.rs` | `git diff -M --stat`, sorted test names identical, release binary behavior identical | M | low (move only) |
| 4 | Introduce `Capabilities` and the narrow traits; `CompositorBackend` becomes the bundle supertrait; GNOME and X11 return capabilities equal to today's behavior | test names identical; mocks in `commands/mod.rs` tests untouched | M | medium (trait surface) |
| 5 | Move setup/teardown out of the backend trait into `Provisioner`; move the GNOME flags out of `InstallArgs`/`UninstallArgs` while keeping the same clap flags | `install --help` and `uninstall --help` byte-identical; `install_notice_cli_test` unchanged | M | medium (clap flatten) |
| 6 | `HelperStatus` replaces the protocol-number checks in neutral commands (leak #9) with the same messages for GNOME | message strings asserted by existing tests (`transform.rs`, `query.rs`, `spawn.rs` tests) unchanged | S | low |
| 7 | `Host` trait: bin dirs, autostart, executable test, launch detach; Linux values identical (leaks #2, #4, #5, #13) | existing path tests (`installer.rs`, `config.rs`, `upgrade.rs`) unchanged; installer fake-curl matrix identical | M | low-medium |
| 8 | `SessionAction` supplied by the backend; texts move to the GNOME backend (leak #8) | `session.rs` order tests unchanged; source guards updated in the same commit | S | low |
| 9 | `update.rs`: asset name from the target triple, desktop check replaced by a backend hook (leaks #6, #7) | update tests unchanged; asset name for Linux identical | S | low |
| 10 | Backend registry with `detect()`; the old order (forced x11, GNOME, else X11) reproduced exactly; `NullBackend` for unsupported hosts (leaks #11, #12) | backend-choice tests; behavior table identical on Linux | S | medium (selection order) |
| 11 | Cross-OS compile: per-target dependency tables, `cargo check` jobs for macOS and Windows runners (compile only) | CI green; Linux artifacts unchanged | M | medium (first time it is ever built off Linux; **unverified** how much breaks) |
| 12 | First new backend (below) behind `SPAWN_AT_BACKEND=<name>`, never auto-selected until its live suite passes | its own live suite; GNOME suites untouched | L | medium |

Steps 1-3 can be done at once and by themselves reduce risk. Steps 4-10 are the structural change. Step 11 should come early enough that new leaks are caught by CI; it can move up right after step 1.

**Which backend first: Sway (i3-IPC), then Hyprland.** Reasons: (1) it needs no helper to install, so it exercises the new `Provisioner` boundary with the *null* case, the opposite of GNOME, which is the best test of the abstraction; (2) a text/JSON socket protocol is easy to write and to fake in unit tests; (3) headless compositor runs make it the most CI-testable real compositor (unverified but commonly done); (4) i3 uses the same IPC, so it may cover a second window manager family for free (unverified); (5) the missing pre-map claim forces the "poll or event, no cloak" path that every non-GNOME backend will need, so building it early validates `Capabilities`. Hyprland second because its `exec [rules]` is the closest to spawn-at's model. X11 is already there and should only be moved, not rewritten. Windows and macOS come last: they need new host services (steps 7, 11) and bring the largest unverified surface.

## 7. Risks and open questions

**Must not change** (the GNOME Wayland contract):

* The D-Bus interface `org.gnome.Shell.Extensions.SpawnAt` and the instruction JSON (`mechanics.rs:42`, `dbus.rs:3-6`), the protocol numbers (`gnome/mod.rs:461-464`), and the extension files (`assets/gnome`, which every release compares byte for byte).
* The ready-wait policy and its Rust/JS parity (`ready_wait.rs` and `assets/gnome/modern/ready_wait.js`, checked by `tests/ready_wait_vectors`), the 30 ms legacy rule, `wait.strategy` / `wait.cap_ms`.
* Placement math (`geometry.rs`, `verify/golden/placement.json`), config keys and file location, CLI flags and `--help` text, exit codes.
* The installer flow just stabilised in beta.3: the log-out question last (`session.rs`), `--clean-install`, the prompts' width rules (`prompts.rs`), `install.sh` behavior.
* The test suites: 273 / 64 / 10 / 404 (4 ignored) / 183 at beta.3; only counts that a step deliberately changes may change, with docs updated in the same commit.

**Risks.** (1) Trait churn breaks the many test doubles in `commands/mod.rs`, `spawn.rs`, `query.rs`; the supertrait bundle in step 4 is meant to absorb that. (2) `clap` flatten changes can alter help text silently; step 5 must diff help output. (3) Moving code between files changes `file:line` citations in docs (`DESIGN_READY_WAIT.md:66` cites `linux/mod.rs:73`); docs need a pass per move. (4) Off-Linux compilation has never been tried; its cost is unknown. (5) Only GNOME 46 on Wayland has ever been run; any restructuring that touches GNOME-shaped code is verified only by unit tests and the owner's live run. (6) Two helper-based backends (GNOME, KWin) will want a shared "deploy and version a helper" abstraction; designing it from one example is a risk, so keep it GNOME-internal until a second helper exists.

**Open questions.** Should `WindowMetadata` and `MonitorLayout` move into `spawn-at-core` so backends can live in separate crates? Should backends be separate crates at all, or modules behind cargo features? What is the supported matrix for Windows window creation timing (is any pre-placement possible)? Is a silent X11 fallback on a non-GNOME Wayland session ever the right behavior, or should it be an explicit error with advice? How should `spawn-at-verify` become backend-aware without growing GNOME-only tools (`gdbus`, `journalctl`) into every spec?

## 8. Findings worth remembering

* (Fixed by step 1; kept as history.) The crate could not compile off Linux: `config.rs:108,151`, `commands/spawn.rs:52`, `commands/config.rs:5` use `platform::linux` unconditionally while the module is `cfg(target_os = "linux")` (`platform/mod.rs:16`); `platform/upgrade.rs:68` uses `std::os::unix` unguarded. No cross target was installed, so this is from reading, not from `cargo check`.
* Backend choice: forced `SPAWN_AT_BACKEND=x11`, else GNOME if `XDG_CURRENT_DESKTOP` contains GNOME and D-Bus connects, else X11 for everything (`linux/mod.rs:73-98`). On Sway/Hyprland/KDE Wayland that means XWayland windows only.
* The neutral command layer is genuinely neutral: it only uses `CompositorBackend`; `gnome::mechanics` is referenced from production code only inside the GNOME driver.
* `run_spawn_with` is one function of about 530 lines (`spawn.rs:58-588`); splitting it is a prerequisite for reasoning about a second ready-signal model.
* `/proc` process ancestry existed twice; step 2 merged it into `linux/process_tree.rs:4-33`.
* Extension protocol numbers (3, 4, 5, 6) are compared in neutral commands (`spawn.rs:101`, `transform.rs:44`, `query.rs:74`; constants `gnome/mod.rs:461-464`).
* Only two of the `supports_*` capability flags are read in production (`transform.rs:18`, `lifecycle.rs:34`).
* Ready-wait exists twice, in Rust (`spawn-at-core/src/ready_wait.rs`) and JavaScript (`assets/gnome/modern/ready_wait.js`), tied together by shared vectors in `tests/ready_wait_vectors`.
* The GNOME extension exposes only a protocol number over D-Bus, no content hash (`assets/gnome/modern/dbus.js`, `extension.js:1529`); "is the running extension current" therefore needs file facts (`logout.rs`).
* `spawn-at-verify` depends only on `spawn-at-core` and talks to the binary as a black box, but its log and display probing is GNOME/Linux tooling (`log_source.rs:39`, `ext_logging.rs:20`, `display.rs:77-85`, `probe.rs:150`).
* `escalate.rs` already has macOS (`sudo install`) and Windows (not implemented) branches (`escalate.rs:23-55`); like the other off-Linux code it has never been compiled.
* Wayland has no generic "move another client's window" protocol; each compositor needs its own IPC or script (public knowledge, unverified here).
