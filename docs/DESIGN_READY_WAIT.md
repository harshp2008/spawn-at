# Design: "wait until the window is ready before placing and revealing it"

Status: design only, 2026-10-13. No product code changed; the only commit is this file. Builds on `TEST_PLAN.md` 6.12 (why the window is revealed at its pre-resize size), 6.13 (frame callbacks), 6.14 (mutter API leads and the arrival-wait simulation), 6.15 and 6.16 (the flash metric).

## 0. Reading guide

Every claim carries a tag. Evidence classes:

| Tag | Meaning |
| :--- | :--- |
| **CONFIRMED (source)** | read in this repository's code |
| **CONFIRMED (measured)** | measured in this session or in stored runs of this repository |
| **CONFIRMED (typelib / symbols)** | read from the installed `Meta-14.typelib`, `libmutter-14.so`, or vendored crates on this machine (mutter 46.2, gjs 1.80.2) |
| **CONFIRMED (docs)** | stated in this repository's docs; the underlying evidence is cited there |
| **HYPOTHESIS (inference)** | my reasoning from the above; not tested |
| **UNVERIFIED (web)** | a WebFetch summary written by a small model; treat as a lead |
| **UNVERIFIED (knowledge)** | from general knowledge of the platform; not checked here |

Session facts for this document: Ubuntu 24.04, `XDG_SESSION_TYPE=x11`, GNOME Shell 46.0, mutter 46.2, gjs 1.80.2, 60.00 Hz display (CONFIRMED (measured), `xrandr`). Nothing in a Wayland session, on GNOME 50, on KDE/wlroots, Windows or macOS was run.

### 0.1 Decisions in one page

1. **One contract, per-window strategies.** The unit of choice is the window, not the session: a native Wayland window, an XWayland window and an X11 window need different "presented" signals even inside one GNOME Wayland session (section 3.1).
2. **The policy and the outcome type live in `spawn-at-core`; the strategy engine is a small sans-IO state machine** (events in, actions out, a virtual clock for tests). GNOME runs it in JS inside the shell because the cloak and the hard limit must stay compositor-side; Rust backends (X11 driver, later Windows/macOS/wlroots) run the Rust build of the same machine. The two builds are checked against one set of golden event scripts (sections 4.1 and 5).
3. **Tuning constants have one source:** `spawn_at_core::ready_wait::Policy`, serialised into the instruction the extension already receives (`WaitForCommit` args). The extension keeps safety defaults only; a test compares them with the Rust defaults.
4. **Resize outcome** `Reached | AdjustedTo(WxH) | TimedOut{last} | Skipped` is additive everywhere: new JSON field, new D-Bus protocol version, new log tags. Exit codes and existing stderr text do not change in the beta (section 2.3).
5. **Native Wayland first, the agreed wait:** wait for the requested size; the first different-size answer ends the exact wait and hands over to the quiet rule; cap 250 ms (150 ms until section 11); hard limit 1500 ms; skip when `allows_resize()` is false; feature-detect by presence, never by version number. The unique-size probe and the learned-minimum cache are designed but kept out of the beta slice (sections 4.3, 4.4, 6).
6. **X11 and XWayland get an observe-only slice first** (log when `damaged` / `thawed` fire) before any wait is built on them, because the "presented" semantics of those signals are unverified (section 3.2).

## 1. Inventory: where readiness, wait and anchor logic lives today

### 1.1 GNOME Shell extension (`assets/gnome/modern/`, `assets/gnome/legacy/`)

All CONFIRMED (source). `modern` = GNOME 45-48 (`metadata.json`: `45,46,47,48`), `legacy` = GNOME 42-44. The two trees are parallel copies (different module syntax, same logic); `commit.js` is 174 lines modern and 152 legacy, `extension.js` 1631 and 1622.

| What | Where (modern) | Role | GNOME-specific? |
| :--- | :--- | :--- | :--- |
| Instruction runner: `Snapshot`, `Cloak`, `SetSize`, `WaitForCommit`, `SetPositionAnchored`, `Uncloak`, `DestroySnapshot`; an unknown instruction throws | `extension.js:661` `_runBatch` | the step sequence of a placement (create/hide/size/wait/anchor/reveal) | contract is neutral, the implementation is GNOME |
| Hide / reveal | `cloak.js` (`CloakManager`: opacity 0, offscreen redirect off, snapshot, 6000 ms watchdog `cloak.js:49`); `extension.js:323-335` `_cloak/_uncloak`; `_reveal` | Clutter actor opacity | GNOME (Clutter) |
| Request size | `extension.js:738` `_stepSetSize` (clamps to floor 100x60, `move_resize_frame`) | `Meta.Window.move_resize_frame` | GNOME (Mutter) |
| Minimum size | `extension.js:300` `_getMinSize` -> `window.get_min_size?.()`; constants `GEOMETRY_FLOOR_W/H = 100/60` (`:33-34`) | returns `[0,0]` on 46 because the method is absent (typelib, below) | GNOME; also wrong for 50 (`[ok,w,h]` return, 6.14) |
| **The wait** | `commit.js:34` `waitForCommit` (`size-changed` + actor `notify::allocation`; quiet timer 30 ms, idle timeout `max(timeout_ms,120)`, hard cap 1500 ms; finishes on `AT_TARGET`, `SIZE_SETTLED`, `TIMEOUT_NO_CHANGE`, `HARD_CAP`); `extension.js:785` `_stepWaitForCommit` | the thing this design replaces | GNOME signals, **neutral algorithm** |
| One-frame waits | `commit.js:144` `waitForFrames(stage, 1, 50 ms)` called from `_stepUncloak` (`extension.js:863, 876`) | lets the stage paint the moved window before the reveal | GNOME (Clutter `after-update`) |
| Late-resize repair before reveal | `extension.js:961` `_reanchorLateResize` (max 2 passes, `LATE_RESIZE_MAX_PASSES`, `:38`), `:929` `_reanchorIfResized`, `:922` `_isToolkitClamped` | re-anchor if the frame size changed since positioning | neutral idea, GNOME calls |
| Anchor | `extension.js:1022` `_computeAnchoredPosition`, `:1033` `_applyAnchoredPosition` | pivot/anchor/margin to x,y from the frame size | **platform-neutral maths** (same maths in `spawn-at-core::geometry`) |
| Reactive anchor after reveal | `extension.js:1062` `_armAnchor`, `:1074` `_onAnchoredSizeChanged` (logs `ANCHOR_CORRECTION` only when the recomputed position differs), `:1098` `_disarmAnchor`, retire after 1000 ms (`ANCHOR_RETIRE_QUIET_MS`) | fixes a late resize after the reveal; it is the flash the verify tool reports | GNOME signals; neutral concept |
| Result back to CLI | `_reveal` emits `SpawnClaimed` `(sbtiiuubs)`: target, success, window id, x, y, w, h, size_raised, error string (empty on success) | the only outcome channel | GNOME D-Bus |
| Per-window client type | `logger.js:271-278`: `get_client_type()`; `X11` under a Wayland compositor is labelled `XWayland` | already distinguishes the window kinds, for logging only | GNOME |
| Protocol version | `extension.js:1361` `ProtocolVersion` = 5 | gate for new behaviour | GNOME |
| Tuning constants (JS) | `COMMIT_QUIET_MS 30`, `COMMIT_MIN_TIMEOUT_MS 120`, `COMMIT_HARD_CAP_MS 1500` (`commit.js:10-12`); `FRAME_WAIT_CEILING_MS 50` (`extension.js:36`); focus pulse constants in `pulse.js` | one of two sources (see 1.2) | neutral values |

Not used anywhere: `Meta.Window.allows_resize` (grep: no use in the repo; CONFIRMED (source)).

### 1.2 Rust CLI (`src/`) and core (`crates/spawn-at-core/`)

| What | Where | Role | GNOME-specific? |
| :--- | :--- | :--- | :--- |
| Declarative intent | `crates/spawn-at-core/src/driver.rs` `Batch`, `Entry`, `Reveal`, `Driver::arm` | what to place; **no readiness concept at all** | neutral |
| Geometry maths | `crates/spawn-at-core/src/geometry.rs` `calculate_rect_from_placement` (`:479`), `apply_anchor`, `clamp_to_bounds`, `compute_screen_anchor` | anchor/pivot/margin to rect | neutral |
| Backend trait | `src/platform/mod.rs:101` `CompositorBackend` (`supports_runtime_transform`, `supports_claim_wait`, `prepare_claim_wait`, `get_window_rect`, `transform_window`, ...) | the platform seam today | neutral trait; capability flags are coarse booleans |
| Instruction synthesis | `src/platform/linux/gnome/mechanics.rs:171` `build_instructions_for_entry` (spawn: `WaitForCommit{timeout_ms:300}`), `src/platform/linux/gnome/driver.rs:289` `transform_window` (`timeout_ms:500`), `Instruction` enum (`mechanics.rs:41-49`) | step list for the extension | GNOME |
| Claim wait | `src/platform/linux/gnome/driver.rs:53-138, 235-265` `GnomeClaimSubscription` (needs protocol >= 2; a failed claim returns an error, a successful one ignores the `error` string) | receives `SpawnClaimed` | GNOME |
| CLI-side settle | `src/commands/mod.rs:362` `settle_window_geometry` (polls `get_window_rect` every 25 ms, 300 ms timeout; "size matches" is `>=` the request), called from `spawn.rs:450` and `transform.rs:339`; `spawn.rs:465-495` re-asserts once if the client re-maximised | second, independent wait on the CLI side, with its own constants | neutral logic, polls the backend |
| Final report | `src/commands/spawn.rs:21` `SpawnOutputJson {window_id,x,y,w,h,rel_x,rel_y,area,size_raised,raised_w,raised_h,reused_existing_window,source}`; `src/diagnostics.rs:570` `verify_and_report_placement` (`[spawn-at] INFO/WARNING/ERROR:` on stderr; `SizeRaisedToMinimum` is INFO) | what the user sees | neutral |
| Native X11 driver | `src/platform/linux/x11/interception.rs:23-` `handle_spawn_interception`: `SubstructureNotify`, pre-map `WM_NORMAL_HINTS` `USPosition\|PPosition` and `_NET_WM_USER_TIME=0`, poll `MapState`, wait until the geometry is above 50x50 plus 40 ms, `move_resize` via `_NET_MOVERESIZE_WINDOW`. **No hide, no sync, no damage** (grep of `x11/*.rs`: no unmap/opacity/cloak/damage/`_NET_WM_SYNC`) | post-map mover | X11 |
| Backend selection | `src/platform/linux/mod.rs:101` `bootstrap`: `SPAWN_AT_BACKEND=x11` forces the X11 driver (`sway` forces the EXPERIMENTAL Sway driver, which has no ready-wait); if `XDG_CURRENT_DESKTOP` contains GNOME the GNOME driver is used (it also runs on a GNOME X11 session), else X11 | **per session** | Linux |
| x11rb features | `Cargo.toml:17` `x11rb 0.14` with only `randr`. `damage` and `sync` features exist in x11rb 0.14.0 (CONFIRMED (symbols), vendored `Cargo.toml`) | a damage/sync listener needs two feature flags | X11 |

Two constant sources exist today: JS (`commit.js`) and Rust (300/500 ms in `mechanics.rs`/`gnome/driver.rs`, 300 ms in `spawn.rs`/`transform.rs`). The JS idle timeout is `max(timeout_ms, 120)`, so the Rust 300/500 only matters above 120 ms. CONFIRMED (source).

### 1.3 What would have to be rewritten per target

| Target | Reusable as is | Rewrite |
| :--- | :--- | :--- |
| **X11 only (any WM)** | geometry maths, `Policy`, `ResizeOutcome`, state machine, JSON/stderr/exit surface | hide (none exists today), request (`_NET_MOVERESIZE_WINDOW` exists), presented signal (damage and/or `_NET_WM_SYNC_REQUEST_COUNTER`, section 3.2), min size (`WM_NORMAL_HINTS`, readable pre-map, section 3.2), reveal; **no resident process**: the CLI must stay alive for the whole wait |
| **wlroots (Sway/Hyprland) / KDE** | as above | new backend per compositor: wlroots has no extension API, only IPC rules (Sway `for_window`, Hyprland window rules) or a layer-shell-free path; KDE has KWin scripts/rules. Hide, presented and min-size are all compositor-specific (UNVERIFIED (knowledge), section 3) |
| **Windows** | geometry maths, policy, outcome, CLI surface | everything platform: Win32 hooks/DWM for hide, `SetWindowPos`, `WM_SIZE`/`WM_PAINT`/`DwmFlush` or equivalent for presented, `WM_GETMINMAXINFO` (app-answered) for min size (UNVERIFIED (knowledge)) |
| **macOS** | same | Accessibility API (`AXUIElement`) for size/position, no supported way to hide another process's window, no presented signal, Spaces (UNVERIFIED (knowledge)); the README already says "needs a permission grant" |

## 2. The contract

### 2.1 Platform-neutral steps

A placement is a transaction over one window. Names are neutral; GNOME's current instruction names map as shown.

| # | Step | Contract | GNOME today |
| :---: | :--- | :--- | :--- |
| 1 | **claim** | identify the window that belongs to this spawn; learn its **kind** (`NativeWayland`, `XWayland`, `X11`, `Win32`, `Cocoa`, ...) and its **capabilities** | `window-created` + `ArmSpawn` match; kind from `get_client_type()` |
| 2 | **hide** | make it invisible before any frame can reach the screen; must be released by a hard watchdog | `Cloak` (+ `Snapshot`) |
| 3 | **request_size(w,h)** | ask for a size; returns at once with what the platform already knows (resizable, fullscreen/tiled, known minimum) | `SetSize` |
| 4 | **wait_presented(policy) -> ResizeOutcome** | wait until the content at the new size has been presented (or the platform's best proxy), under `Policy` caps | `WaitForCommit` (to be replaced) |
| 5 | **anchor(size)** | compute and apply the position for the **outcome size**, not the requested size | `SetPositionAnchored` |
| 6 | **reveal** | un-hide in the same tick as the last position check | `Uncloak` / `_reveal` |
| 7 | **verify** | re-read the final rectangle, keep a reactive anchor armed for a short time, report the outcome | reactive anchor; `SpawnClaimed`; CLI settle |

Step 4 is the only new contract; 5-7 exist. The requirement that step 5 uses the outcome size is the fix for "every corrected run had the anchor computed for 400x300" (6.12).

### 2.2 Resize outcome

```text
enum ResizeOutcome {            // #[non_exhaustive]
    Reached    { size: (w,h), waited_ms, evidence },
    AdjustedTo { size: (w,h), reason: Min | Max | Increment | WorkArea | Unknown, waited_ms, evidence },
    TimedOut   { last_size: (w,h), waited_ms, phase: Cap | Hard },
    Skipped    { reason: NotResizable | Fullscreen | Tiled | NoSizeRequested | Unsupported },
}
enum Evidence { FrameEquals, SizeChanged, SyncThaw, Damage, Probe, GraceElapsed }   // #[non_exhaustive]
```

*Implemented subset (section 8): `evidence`, `AdjustedTo.reason` and the `NoSizeRequested` / `Unsupported` skip reasons are not in the machine's outcome yet, because it sees only sizes and time; the enum is `#[non_exhaustive]` so a backend can add them.*

`Reached`: the presented size equals the request (tolerance 0 px; 1 px for backends that round). `AdjustedTo`: the window answered with a different size (the app's minimum, a clamp, a size increment such as a terminal grid, a work-area clamp) and that size is stable under the quiet rule. `TimedOut`: no answer inside the cap (`phase: Cap` when the optional probe also failed or is disabled, `Hard` when the hard limit ended it); `last_size` is what the window had. `Skipped`: no wait was attempted. The fourth variant is the only addition to the three requested; without it "not resizable" has to be forced into `TimedOut`, which would misreport a dialog as a hung app.

Mapping from today's `COMMIT_RESOLVED` reasons (CONFIRMED (source), `commit.js`): `AT_TARGET` -> `Reached`; `SIZE_SETTLED` -> `AdjustedTo` **or a stale answer** (the case behind the flash: the first sample is the app's initial size; this is exactly what the new rule separates); `TIMEOUT_NO_CHANGE` -> `TimedOut{Cap}`; `HARD_CAP` -> `TimedOut{Hard}`.

### 2.3 How the outcome surfaces; minimum compatible shape for the beta

Rule for the beta: **additive only**. A script written against today's CLI must see no change.

| Channel | Beta shape | Why it does not break users |
| :--- | :--- | :--- |
| **Exit code** | unchanged: 0 whenever the window was placed, whatever the outcome. `TimedOut`/`AdjustedTo` do not change it | the exit code for a mis-positioned window is undefined today (TEST_PLAN 6.4 row 5, 6.10 "skipped as UNDEFINED"); defining a non-zero code now would turn today's passes into failures. An opt-in `--strict-size` (non-zero on `TimedOut`) can come later |
| **stderr** | no new line for `Reached`. `AdjustedTo`: the existing `INFO` (`SizeRaisedToMinimum`, `diagnostics.rs:150`) already covers the raised-to-minimum case; other reasons add one `INFO` line. `TimedOut`: one `WARNING: window did not answer the size request within <ms>; placed for <WxH>` only when the final rectangle misses the target (today's `Window mapped at ... expected` warning already fires in that case, so the new line replaces nothing and adds one reason) | `[spawn-at] INFO/WARNING/ERROR:` prefixes are the contract the verify specs match (`stderr_absent`); a new WARNING can only appear in runs that already warn |
| **JSON (`--json`)** | add `"resize": {"outcome":"reached"\|"adjusted"\|"timed-out"\|"skipped","w":..,"h":..,"waited_ms":..,"evidence":"..","reason":".."}` and `"schema":1`. Keep `size_raised`, `raised_w`, `raised_h` (derivable: adjusted and larger). Consumers must treat unknown `outcome`/`evidence`/`reason` strings as `adjusted`/opaque | `SpawnOutputJson` is read with serde and `#[serde(default)]` fields already exist for `raised_*`; the verify harness reads stdout as a generic JSON value |
| **D-Bus** | **Decided at wiring time (supersedes the earlier plan to overload `SpawnClaimed.error`):** `ProtocolVersion` 5 -> 6 and a new typed signal `ResizeOutcome(target_id s, outcome s, final_w u, final_h u, waited_ms u, reason s)`, emitted just before `SpawnClaimed` for the same target. `SpawnClaimed` is unchanged. The CLI subscribes to `ResizeOutcome` only when the extension reports protocol >= 6 and attaches the newest outcome for the claimed target, without waiting for it; with protocol 5 there is no outcome and `resize` is left out of the JSON. Windows waited for with the legacy rule send no signal. (The earlier text: policy args in `WaitForCommit`, outcome as a JSON string in the `error` field of a successful `SpawnClaimed`.) | `SpawnClaimed` is `(sbtiiuubs)`; changing the signature would break every installed CLI/extension pair, and a new signal does not: an old CLI never subscribes to it, and an old extension never emits it. New instruction *names* would break (the extension throws on an unknown instruction), so no new instruction is added |
| **Logs** | keep every existing tag (`COMMIT_SAMPLE`, `COMMIT_RESOLVED` with its `reason`; the verify tool parses them: `trace/phases.rs`). Add `READY_WAIT_START strategy=.. kind=.. target=WxH cap=.. hard=..`, `READY_WAIT_ANSWER size=WxH t=..`, `READY_WAIT_DONE outcome=.. waited_ms=.. evidence=..` | additive lines; the analyser ignores unknown tags (CONFIRMED (source), parser regex accepts any tag) |
| **Config / flags** | no user-facing flag in the beta. A hidden developer switch `--ready-wait=<variant>` on the CLI selects the strategy and is carried in the instruction args, so A/B runs need no extension reload and no file in `$XDG_RUNTIME_DIR` (the earlier idea in 6.13) | nothing to document or support |

`Policy` and `ResizeOutcome` are `#[non_exhaustive]` and serialised with a `schema` number so they can grow in Stage 4 (new reasons, new evidence kinds, a typed D-Bus signal) without a breaking change.

## 3. Backend capability matrix

Columns: **hide** / **request size** / **detect "presented"** / **minimum knowable?** / **fallback**. "Presented" means the content at the requested size has been drawn by the client, which is what the user sees; the window manager's rectangle is not that on every platform (3.2).

| Target | hide | request size | detect "presented" | minimum size knowable? | fallback | Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **GNOME 46 Wayland, native client** | actor opacity 0 (`CloakManager`) | `move_resize_frame` | the frame rect follows the client's committed buffer, so `size-changed` to the requested size is the signal. `WindowActor` has `damaged`, `first-frame`, `thawed` (semantics untested) | **no**: `get_min_size`/`get_max_size` are absent on 46; learn it by observation (4.4) | cap, then the quiet rule | hide, request, signal: CONFIRMED (source, 6.12-6.13 measured); typelib: CONFIRMED (typelib); minimum absent: CONFIRMED (typelib, 6.14) |
| **GNOME 42-44 (`legacy`)** | same | same | same Mutter model; `Meta-10..12` API | `get_min_size` likely absent | same | the extension tree exists and is shipped (CONFIRMED (source)); the 46-only facts above were **not checked** on 42-44: UNVERIFIED (knowledge) for `get_client_type`, `allows_resize`, `thawed`, `damaged` |
| **GNOME 45, 47, 48 (`modern`)** | same | same | same | absent per 6.14 (47.0, 48.0 headers) | same | `metadata.json` lists them (CONFIRMED (source)); 45/47/48 not run: UNVERIFIED |
| **GNOME 50** | same | same | same, plus mutter 50 may offer external constraints | **`get_min_size`/`get_max_size` and `add_external_constraint` appear at 50** | use them when present (feature-detect), else observe | UNVERIFIED (web, 6.14 summaries). `shell-version` does not list 50 (CONFIRMED (source)); `_getMinSize` mishandles the `[ok,w,h]` return (CONFIRMED (source) + HYPOTHESIS (inference) about GJS out-argument convention) |
| **GNOME on X11 (46, this session)** | actor opacity (same Clutter path) | `move_resize_frame`; the manager sets the rect at once | **the frame rect is not the signal**: `AFTER_SET_SIZE` reads the requested size 0.06-0.2 ms after the request and `COMMIT_RESOLVED` is `AT_TARGET` 0.15 ms later (CONFIRMED (measured), 60 stored traces in 6.15). Candidates: `thawed`/`damaged` on the actor, or a client damage listener | **yes, before map**: `WM_NORMAL_HINTS` program minimum, read with `xprop` (CONFIRMED (measured), 3.2) | grace of fixed length (HYPOTHESIS, no data) | see 3.2 |
| **XWayland window inside a Wayland session** | same actor path | `move_resize_frame`; X11 semantics (HYPOTHESIS (inference): `MetaWindowXwayland` is an X11 window, the rect is the manager's) | as GNOME on X11 (damage/sync); plus mutter freezes commits for these windows: `meta_window_xwayland_thaw_commits`, `meta_window_actor_can_freeze_commits` exist (CONFIRMED (symbols)); meaning HYPOTHESIS | `WM_NORMAL_HINTS` is visible to the X server; whether mutter exposes it to the extension without `get_min_size` is **UNVERIFIED** | as X11 | 3.1 |
| **wlroots (Sway, Hyprland), KDE** | no extension API. wlroots: compositor rules/IPC at map time; KDE: KWin script/rule; hiding another client's window before first frame is compositor-specific | Sway/Hyprland IPC commands; KWin script `frameGeometry` | generic Wayland: `xdg_toplevel.configure` -> `ack_configure` -> `wl_surface.commit` of that size is visible only to the compositor; an external tool sees it via toplevel-management events at best | the compositor knows `xdg_toplevel.set_min_size`; external access is compositor-specific | apply the placement through a compositor rule at map (no wait) | UNVERIFIED (knowledge); README says "Not implemented" for Hyprland, Sway, KDE (CONFIRMED (docs)) |
| **Windows** | no supported cross-process hide before first paint; options (CBT hook that edits the create struct, layered-window alpha, DWM cloak) each have limits | `SetWindowPos` | `WM_SIZE` then the first `WM_PAINT` is the app's, not observable cross-process without a hook; `DwmFlush` is a compositor tick | the app answers `WM_GETMINMAXINFO` on demand; probe by request-and-read | probe-by-observation; fixed grace | UNVERIFIED (knowledge); README: "nothing places it before first paint" (docs) |
| **macOS** | no supported hide for another process's window (private SkyLight alpha exists) | Accessibility `AXSize`/`AXPosition` (permission grant) | none public | Accessibility exposes no minimum; probe by request-and-read | fixed grace; Spaces complicate | UNVERIFIED (knowledge) |
| **X11, any WM, no GNOME (`X11Driver`)** | none today (no unmap, no opacity) | `_NET_MOVERESIZE_WINDOW` | client damage (XDamage) and `_NET_WM_SYNC_REQUEST_COUNTER` are the available proxies; neither is used today (CONFIRMED (source)) | `WM_NORMAL_HINTS` | as above | CONFIRMED (source) for what exists |

### 3.1 The backend choice is per window (item 3a)

**How an extension tells the kinds apart.** `Meta.Window.get_client_type()` exists in the installed typelib and `Meta.WindowClientType` is `{WAYLAND: 0, X11: 1}` (CONFIRMED (typelib)). `logger.js:271-278` already calls it and labels an `X11` client under a Wayland compositor `XWayland` (CONFIRMED (source)). `Meta.is_wayland_compositor()` exists (CONFIRMED (typelib)). So the kind is known at claim time with no new API. The same typelib has no `get_client_pid` (`get_pid` exists and is documented as spoofable; 6.12), so kind detection must not depend on pid. Stored traces: 230 732 `proto=Wayland` lines and 5 132 `proto=X11` lines; **no `proto=XWayland` line has ever been recorded** (CONFIRMED (measured), grep of `verify-results/`), so the XWayland label has never been exercised.

**Does the frame rect behave like native X11 for XWayland windows?** HYPOTHESIS (inference): yes. Mutter's XWayland windows are X11 windows managed by the same window code (the symbols `meta_window_xwayland_*` are an X11-window subclass layer; CONFIRMED (symbols) only that they exist), so the manager sets the rect immediately. Not tested: this machine has no Wayland session. The fixture can create one on demand: `fixture --protocol x11` inside a Wayland session (it needs `DISPLAY`, which XWayland provides; `resolve_backend` in `fixture.rs` requires it; CONFIRMED (source)). `WINIT_UNIX_BACKEND=x11` does **not** work with the pinned winit: the string does not occur in winit 0.29.15's source (CONFIRMED (symbols), grep of the vendored crate); use the existing `--protocol x11` flag.

**Does `_NET_WM_SYNC_REQUEST` apply to XWayland windows?** HYPOTHESIS (inference): yes, because it is an X11 client protocol and mutter freezes and thaws commits for these windows (`xwayland_window->freeze_count`, `meta_window_xwayland_thaw_commits`, `meta_window_actor_can_freeze_commits`: CONFIRMED (symbols) that they exist). Whether a given XWayland client answers it depends on the client (3.2 data comes from a native X11 session).

**Which popular apps run as XWayland on a GNOME 46 Wayland session by default** (UNVERIFIED (knowledge), version dependent, none checked here): Electron apps (VS Code, Slack, Discord, Teams, Claude desktop, Obsidian) unless started with an Ozone Wayland flag; Chrome/Chromium unless the Ozone hint is set; Steam and most games via SDL2/Proton; Wine; JetBrains IDEs; Java Swing/AWT; Zoom; some Qt5 apps. Native Wayland by default: GTK3/GTK4 apps, Firefox (since 121), Qt6 apps, most GNOME apps.

**Consequence for strategy order.** On the default Ubuntu session (Wayland) a large share of the windows users place with this tool are X11-kind. So "X11/XWayland" is not a legacy path that can wait for Stage 4: the strategy engine must select per window from day one, and the beta must at least keep X11-kind windows on the old path **explicitly** (by kind), rather than accidentally by an unknown-API fallthrough. Order by kind:

| Window kind | Strategy order (first that the window supports) |
| :--- | :--- |
| `NativeWayland` | 1 exact-size wait -> 2 first different-size answer then quiet -> 3 cap -> 4 optional probe (4.3) -> 5 hard limit |
| `XWayland` / `X11` (extension) | 1 sync client: wait for the actor's `thawed` (UNVERIFIED) -> 2 `damaged` after the size change -> 3 fixed grace (>= 1 frame period) -> 4 hard limit. The frame rect is **not** consulted for "arrived" (it is immediate) |
| `X11` (Rust `X11Driver`, no extension) | 1 `_NET_WM_SYNC_REQUEST_COUNTER` alarm -> 2 XDamage on the client window -> 3 grace -> hard limit |
| `Win32`, `Cocoa` | observation (request-and-read) with grace; no presented signal |

### 3.2 GNOME on X11 (item 3b), measured in this session

**Which clients advertise `_NET_WM_SYNC_REQUEST` (CONFIRMED (measured), `xprop`, read-only, on this X11 session; the probe apps were opened for about 2 s and closed):**

| Window | Toolkit | `WM_PROTOCOLS` includes `_NET_WM_SYNC_REQUEST` | `_NET_WM_SYNC_REQUEST_COUNTER` | `WM_NORMAL_HINTS` minimum |
| :--- | :--- | :---: | :--- | :--- |
| **spawn-at fixture** (`--protocol x11`) | winit 0.29.15 (Xlib/x11rb) | **no** (only `WM_DELETE_WINDOW`, `_NET_WM_PING`) | not set | as passed by `--min-width/--min-height`: 300x200 for `--min-width 300 --min-height 200` |
| Nautilus (running) | GTK4 | yes | **two** counters (extended sync) | 360x380 |
| gnome-text-editor | GTK4 | yes | two counters | 513x222; `_GTK_FRAME_EXTENTS` 61,61,55,67 (large client-side shadow) |
| gnome-calculator | GTK4 | yes | two counters | not read |
| Google Chrome (running) | Chromium | yes | one counter | 849x193 |
| Claude desktop (running) | Electron | yes | one counter | 790x553 |
| Antigravity IDE (running) | Electron | yes | one counter | 500x338 |
| xterm, xclock, xcalc | Xt/Xaw | **no** (only `WM_DELETE_WINDOW`) | not set | not read |

Two facts follow. (1) **The fixture is a worst-case client on X11**: it does not take part in sync, and winit 0.29.15 contains no sync-request code at all (CONFIRMED (symbols), grep of the vendored crate). That matches the measurement of 6.15 (X11, debug fixture: flash 19 of 20, release fixture: 0 of 10); GTK4, Chromium and Electron clients take part, so the X11 flash seen with the fixture may be much rarer with them. That last sentence is a HYPOTHESIS: no flash measurement exists for a sync-capable X11 client. (2) **The minimum size is knowable before map on X11** from `WM_NORMAL_HINTS` for every toolkit above; on native Wayland it is not (the matrix row for GNOME 46). With a caveat: GTK4's hint is for the client area while the window has a 61/55 px invisible shadow, so frame and buffer rectangles differ (`_GTK_FRAME_EXTENTS`); a wait that compares sizes must say which rectangle.

**Does mutter already delay the resized frame until the client repaints?** What is established (CONFIRMED (symbols), strings and exports of `libmutter-14.so.0` 46.2): `_NET_WM_SYNC_REQUEST` and `_NET_WM_SYNC_REQUEST_COUNTER` handling, a log line "Window has _NET_WM_SYNC_REQUEST_COUNTER 0x%lx (extended=%s)", a `[mutter] sync_request_timeout`, `meta_window_actor_freeze` / `meta_window_actor_thaw` (exported), the `thawed` signal on `Meta.WindowActor`, and `_NET_WM_FRAME_DRAWN` / `_NET_WM_FRAME_TIMINGS` advertised in `_NET_SUPPORTED` (CONFIRMED (measured), `xprop -root`). What is **not** established: that the freeze delays only what is *drawn* while `get_frame_rect()` is already the new size. HYPOTHESIS (inference): that is how it works (mutter sets the rect, sends the sync request, keeps showing the old texture until the counter is updated, then thaws). Consequence if true: for sync-capable clients the compositor already hides the stale content once the actor is thawed, so a flash on X11 would need a non-sync client; and "frame == target" still says nothing about being presented. Test: log `thawed` timestamps next to `UNCLOAK` for a GTK4 app on X11 (section 5.4).

**Can an extension observe a paint or damage signal on the window actor?** Present in the typelib (CONFIRMED (typelib)): `Meta.WindowActor` signals `first-frame`, `effects-completed`, `damaged`, `thawed`; methods `has_damage()`, `freeze()`, `thaw()`, `get_texture()`; `Meta.ShapedTexture` signal `size-changed`. Not present: `is_frozen`. When each fires relative to a client commit or an X11 repaint has **not** been tested (6.13: "not tested live"); that is the first thing the observe-only slice (6) measures.

**What a Rust-side X11 damage listener would look like** (HYPOTHESIS (inference); needs `x11rb` features `damage` and `sync`, both present in 0.14.0 (CONFIRMED (symbols))):

```text
conn = x11rb connect (own connection; select SubstructureNotify on root to find the window)
win  = the claimed window; read WM_NORMAL_HINTS (min size) and _NET_WM_SYNC_REQUEST_COUNTER (0, 1 or 2 ids)
request size (_NET_MOVERESIZE_WINDOW)
if counter present:  create_alarm(counter, PositiveComparison, last_value+1)  -> AlarmNotify == client finished a frame
damage = damage_create(win, ReportLevel::NonEmpty)                           -> DamageNotify == client drew into the window
loop until policy.hard:
    ConfigureNotify w/ size  -> note answered size
    AlarmNotify | DamageNotify with answered size == target (or quiet) -> presented
    damage_subtract(damage)  to re-arm
```

It sees the *client's* drawing, independent of the compositor, which is what is wanted; it cannot see when the compositor shows it, and on GNOME the hide/reveal lives in the extension, so the listener would have to tell the extension (an extra D-Bus round trip, 0.3 ms idle per 6.12) or run in the extension through the actor's `damaged` signal instead. For the standalone `X11Driver` (no extension, no hide) the listener is the only place it can live. Given the next point, build it last.

**Should GNOME-on-Xorg be a design driver?** UNVERIFIED (web): the mutter NEWS lists "Disable X11 backend by default" (49.alpha.0) and "Drop the X11 backend" (50.alpha); GNOME 50's release page says nothing about X11 (the page was fetched; the summary quoted nothing). If that holds, GNOME 50 has no Xorg *session* but keeps XWayland, so per-window X11 handling (3.1) remains necessary while "GNOME on Xorg" shrinks to GNOME 42-48 on distributions that still ship it. Decision D6 below.

## 4. Placement of logic

### 4.1 Rust core versus a backend

| Belongs in `spawn-at-core` (new `ready_wait` module) | Belongs in a backend |
| :--- | :--- |
| `Policy` (all numbers), `ResizeOutcome`, `Evidence`, `WindowKind`, `Capabilities` | how to hide, how to request, which signals exist, how to read a minimum |
| the **state machine**: pure `fn step(state, event, now) -> (state, actions)` with events `SizeSeen{w,h}`, `Presented`, `Tick`, `ProbeAnswered` and actions `Request(w,h)`, `StartTimer`, `Finish(outcome)`; no I/O, no clock; unit-testable with a virtual clock | feeding events: Mutter signals (JS), x11rb events, Win32/AX polling |
| strategy selection from `(WindowKind, Capabilities)` | `fn capabilities(&self, window) -> Capabilities` on the backend trait, replacing today's coarse booleans for this purpose |
| the learned-minimum **interface** (`MinSizeStore` trait) and its invalidation rules | where the store lives (4.4) |
| outcome-to-surface mapping (JSON `resize`, diagnostics text, log tag text) | transport (D-Bus, in-process) |

**Where it runs on GNOME.** In the shell, in JS. Reasons: the hard limit and the watchdog must be enforced where the cloak lives, so a dead or starved CLI cannot leave a window hidden (the cloak watchdog is 6000 ms, `cloak.js:49`); a Rust loop on the CLI side would add a D-Bus hop to every event and would be scheduled late under load, which is exactly when the wait matters (CONFIRMED (docs), 6.12-6.13: the shell and CPU-starved apps dominate the tail). The JS build is a port of the Rust machine (size: HYPOTHESIS (inference), about 100 lines). **Duplication control:** a directory of golden event scripts (`tests/ready_wait_vectors/*.json`: a list of timed events and the expected actions/outcome) is executed by a Rust test against the core machine and by a `node --test` against the JS machine. The repository already runs both kinds of tests (CONFIRMED (source), `tests/*.js`, 70 pass in 6.16). Divergence then fails a test instead of shipping.

**Two extension trees.** `legacy` (42-44) and `modern` (45-48) are copy-forked. The machine is written once as a plain JS module with no `gi://` or `resource:///` imports (inject `timers`, `now`, and the event source), and each tree imports it; `CloakManager` already takes injected timers "for testing" (`cloak.js` header). HYPOTHESIS (inference): this keeps the fork from growing.

### 4.2 Tuning constants: one source

`spawn_at_core::ready_wait::Policy::default()` is the only definition:

| Field | Meaning | Proposed default | Today |
| :--- | :--- | ---: | :--- |
| `cap_ms` | longest wait for any answer | 250 | JS idle timeout `max(timeout_ms,120)` |
| `quiet_ms` | no further size change for this long ends the quiet phase | 30 | `COMMIT_QUIET_MS` 30 |
| `hard_ms` | unconditional end of the wait | 1500 | `COMMIT_HARD_CAP_MS` 1500 |
| `frame_ceiling_ms` | per one-frame wait before reveal | 50 | `FRAME_WAIT_CEILING_MS` 50 |
| `probe` | `Off` / `On{size, cap_ms}` | `Off` in the beta | - |
| `x11_grace_ms` | fixed grace for X11-kind windows if no signal | unset until measured | - |
| `late_resize_passes` | pre-reveal re-anchor passes | 2 | `LATE_RESIZE_MAX_PASSES` 2 |

*Implemented subset (section 8): `Policy` currently holds `cap_ms`, `quiet_ms`, `hard_ms`, the three numbers the machine consumes; `frame_ceiling_ms`, `late_resize_passes`, `probe` and `x11_grace_ms` join it when the code that uses them is wired.*

Delivery: serialised into the `WaitForCommit` args of the instruction JSON (`{"timeout_ms":300, "policy": {...}}`; old extensions read only `timeout_ms`). The extension keeps the numbers above as *safety defaults* for the case where `policy` is absent (old CLI), and one test asserts `JS defaults == Rust defaults` through a generated JSON file. The CLI's own poll constants (`settle_window_geometry`: 25 ms poll, 300 ms) read `Policy` too, or the CLI-side settle is removed once the outcome arrives in `SpawnClaimed` (decision D7). The verify tool keeps its own measurement constants (`SETTLE_MS` 1300, the 1500 ms flash watch): those are harness values, not product policy.

The source for the original numbers (cap 150 ms, raised to 250 ms in section 11) is the simulation in 6.14: cap 150 ms with the hybrid rule avoided 102 of 134 loaded flashes at a mean 33.5 ms extra and no cost at idle (labelled there as a simulation on stored runs; arrival times are approximations).

### 4.3 The wait for native Wayland (agreed design, as a state machine)

```text
request(w,h):
  if !feature(allows_resize) -> proceed  else if !window.allows_resize() or fullscreen or tiled -> Skipped{..}
  if frame == (w,h) -> Reached(0)
  target = (w,h)             // see 4.4: the learned minimum only changes what we expect, never what we request
  start cap timer (cap_ms), hard timer (hard_ms), phase = EXACT

EXACT   on size-changed:  frame == target            -> Reached
                          frame != previous frame    -> answered = frame ; phase = QUIET ; (re)start quiet timer
        on cap:           no answer                  -> if probe enabled: PROBE  else TimedOut{last, Cap}
QUIET   on size-changed:  frame == target            -> Reached
                          other change               -> answered = frame ; restart quiet timer
        on quiet timer:   -> AdjustedTo(answered)    // today's rule, but started by an *answer*, not by the first allocation
PROBE   (native Wayland only, once, opt-in) request P = a unique size (e.g. 500x600, different from current, target, and any seen size)
        wait for EXACTLY P within probe cap:
            P arrives  -> app is alive: resend the real request ; wait cap2 for any change
                              change -> as EXACT/QUIET ;  no change -> AdjustedTo(current, reason Min) and (4.4) learn min
            P not exact (clamped) -> alive but constrained: AdjustedTo(answer) (grow-clamping app)
            nothing   -> TimedOut{last, Cap}
any phase, on hard timer: -> TimedOut{last, Hard}  (reveal anyway; watchdog is the backstop)
```

Rules that apply to every phase: the first `allocation` sample is **not** an answer (6.12 H1: it was the cause of the stale `SIZE_SETTLED`); only a `size-changed` to a size different from the one before the request is. `allows_resize()` skip is the 6.14 edge case "App ignores the request". Size-increment apps (terminals, 494x292 for a 500x300 request, observed in 6.12) end in `AdjustedTo{Increment}` through the QUIET phase, never in a wait for equality. Feature detection: `typeof window.allows_resize === 'function'`, `typeof window.get_client_type === 'function'`, `window.get_min_size` called through a normaliser that accepts both `[w,h]` and `[ok,w,h]`, `actor.connect('thawed' | 'damaged')` inside `try`, the stage `after-update` as today. **Never** branch on `Shell.version`/`Config.PACKAGE_VERSION`; log them only.

Why the probe exists: without it "no answer by the cap" cannot tell a slow app from one that refuses (at its minimum). Its cost is real and is why it is off by default: it resizes the window twice under the cloak, and an app that persists its size on every configure could store the probe size if killed mid-probe (gnome-text-editor remembers its window size: 6.13; HYPOTHESIS that it writes on every configure). It needs prototype data before it is turned on (5.3).

### 4.4 Learned-minimum cache

Purpose: turn the "below-min, minimum unknown" case (never equals the request, costs the cap on every spawn) into the "minimum known" case, which the 6.14 simulation puts at 19 ms mean extra and 5.3 flashes avoided per ms against 1.8 without it.

**Use rule (important): the cache changes only what the wait *expects* (`expected = max(request, cached_min)`), never what is *requested*.** If the cache is stale-high (the app lowered its minimum) and we requested `cached_min`, we would make the user's window bigger than asked. Requesting the real size and expecting the cached minimum is wrong only by one wasted quiet phase.

| Question | Proposal |
| :--- | :--- |
| Memory or file? | **Beta: nothing** (the probe is also off, so nothing produces entries). **First implementation: memory only, inside the extension** (a `Map` that lives as long as the shell session; GNOME is the only resident backend). **Stage 4: a small file** `~/.local/state/spawn-at/min-sizes.json` (same state dir the logger already uses, `logger.js:79-95`) for non-resident backends (X11 driver, Windows, macOS) where the CLI process is the only process. Interface `MinSizeStore` so the choice is not in the core |
| Key | app identity from what the backend already has: sandboxed app id, else GTK application id, else `wm_class`; plus `WindowKind` and toolkit hint if known. Never the title |
| Value | `{min_w, min_h, learned_at, source: Observed\|Hint\|Api, evidence_count}` |
| What may create an entry | only a *refusal observed by the probe* (request below `min` did not shrink, probe showed the app alive) or an API/hint value (`get_min_size` on 50, `WM_NORMAL_HINTS` on X11). Not a plain `AdjustedTo`: a work-area clamp, a tiled/maximised state or a monitor edge also adjusts |
| Invalidation | drop on contradiction: any later answer smaller than the cached minimum, or `Reached` at a size below it; drop all entries of an app id when its executable identity changes (hash of exe path + mtime where readable, else a 7-day TTL); global TTL 30 days; drop everything when the toolkit-reported kind changes. Electron and GTK change their limits between versions, which is why the identity check exists (HYPOTHESIS (inference) that exe mtime tracks the version for packaged apps, not true for Flatpak/Snap re-mounts) |
| Safety | the cache is advisory; a wrong entry costs one quiet phase, never a wrong placement |
| Privacy | stores app ids and two integers; no titles, no paths in the file (the identity hash is a hash) |

## 5. Test seams and acceptance

### 5.1 What proves a change works

The verify tool already measures the thing, on the client side: the fixture writes `FIRST_PAINT`, `RESIZED`, `MOVED` in `CLOCK_MONOTONIC` (6.15), a `ClockAnchor` ties the extension's `+N ms` to it (half-width 1.5-3.0 ms measured), and each run yields `flash` (numeric, screen independent), `flash_events[].lead_ms`, `flash_certain` / `flash_possible`, `visible_probability` (6.16). A readiness change is accepted when these numbers move, not when a log line looks right. Cloak cost comes from the existing `COMMIT_RESOLVED`/`UNCLOAK` timestamps (`cloak_duration_ms` in the trace report). The ground truth does not depend on the new code (it reads the fixture's own clock), so it can fail the new code.

### 5.2 Numeric acceptance criteria (proposed; HYPOTHESIS that they are achievable)

Baselines from stored data (CONFIRMED (stored data; raw results are not kept in the repository), 6.12-6.13): 22-thread load, corrected after the reveal in 134 of 320 g3 runs (42%) and 10 of 40 fixture-trace runs (25%); idle about 1% (3 of 320). Cells are 100 isolated runs (regime (a) of 6.10); the pass rule uses the Wilson 95% upper bound (computed here: 0/100 -> 3.7%, 1/100 -> 5.4%, 5/100 -> 11.2%, 10/100 -> 17.4%; 40 runs would not separate 25% from 10%: 4/40 -> 23.1%).

| # | Condition (release-build fixture, native Wayland, GNOME 46) | Criterion |
| :---: | :--- | :--- |
| A1 | idle, legal shrink / grow / at-min, 100 runs each | numeric flash <= 1 of 100 |
| A2 | 22-thread `burn-cpu`, legal shrink and grow, 100 runs each | numeric flash <= 5 of 100 (baseline 25-42%), and p95 `lead_ms` <= 50 ms for the rest |
| A3 | idle, compliant sizes: cloak cost | median `UNCLOAK - CLOAK` increase <= +5 ms against the baseline commit; p95 <= 100 ms (the existing provisional budget, 6.12/5) |
| A4 | idle, request below the app's minimum (fixture `--min-width/--min-height` above the request), **first spawn** | median extra <= +40 ms (6.14: cap-bound cost 41-90 ms at C=100-150 without a known minimum) |
| A5 | same, **after** the minimum is learned (once the cache exists) | median extra <= +5 ms and a net saving against today's 30 ms quiet rule |
| A6 | hung app (fixture stalls its event loop 3 s) | reveal at `hard_ms` +50 ms in 100 of 100 runs; no run left cloaked; exit code 0; one WARNING |
| A7 | fixed-size / not-resizable app | `Skipped`; cloak cost equal to the no-resize path (<= +5 ms) |
| A8 | regression | all existing tests (352 passed, 4 ignored with `tui,fixture` at `f26cef7`, plus the new ones), `node --test` 70, and `COMMIT_RESOLVED` tags still emitted |

A2 and A3 trade against each other (6.14: more waiting avoids more flashes); the table fixes both so neither can be bought by giving up the other.

### 5.3 Fixture-based prototype plan for the probe stages

Fake apps are fixture flags (harness only, `crates/spawn-at-verify/src/bin/fixture.rs`; none of these exist yet except the minimum flags, CONFIRMED (source)):

| Fake app | Fixture setting | What it exercises |
| :--- | :--- | :--- |
| answers at once | default (6.12: median 9-17 ms at idle) | EXACT -> `Reached` |
| slow-answering | new `--resize-delay-ms N`: do not repaint on `Resized` for N ms (on Wayland the frame follows the committed buffer) | EXACT with answer after cap; the slow-vs-refusing split |
| refusing / default equals minimum | `--min-width 400 --min-height 300` (existing) with a smaller request | no `size-changed` at all: cap, PROBE, `AdjustedTo{Min}` |
| clamping the grow | new `--max-width/--max-height` (winit `with_max_inner_size`) | probe answer not exactly P: `AdjustedTo` |
| size increment | new `--resize-increment WxH` | `AdjustedTo{Increment}`, QUIET path |
| hung | new `--stall-after-map-ms N` (sleep in the event loop) | hard limit, watchdog |
| not resizable | min == max == initial | `Skipped` |

Plan, each step ends with numbers from the existing metric:

1. **P0 - machine only.** Rust state machine + virtual clock + the golden vectors; the JS port passes the same vectors in `node --test`. No GNOME involved. Includes the cases above as event scripts.
2. **P1 - observe only (extension).** Log `READY_OBSERVE` lines: `size-changed`, `damaged`, `thawed`, `first-frame` timestamps next to `UNCLOAK`, on X11 and Wayland, native and XWayland; no behaviour change. This answers "when does each signal fire" (UNVERIFIED today) and is the data the X11/XWayland strategies need.
3. **P2 - machine in the extension behind the hidden `--ready-wait=<variant>`** (variants: `old`, `exact`, `exact+probe`), carried in the instruction args. Run the fixture matrix above x {idle, 11, 22 threads} x 100 runs; compare variants in one login.
4. **P3 - probe on/off decision** from P2: keep the probe only if it separates slow from refusing without a visible side effect and without storing a wrong window size in apps that persist it.

**XWayland test.** In a Wayland session run the fixture as an X11 client: spec `subject.args = ["--protocol","x11", ...]` (the flag exists, `fixture.rs`), `requires.backend` for the protocol, and the harness's protocol tracking (it records the protocol actually used; "A test requiring native Wayland fails or skips if the window was mapped under XWayland", `verify/README.md` section 7, CONFIRMED (docs)). The extension's trace shows `proto=XWayland` for it (`logger.js:278`). Cannot run on this X11 session; the result is UNVERIFIED until a Wayland session is available. The fixture does not take part in sync (3.2), so it models the non-sync XWayland client; a sync-capable XWayland client (Chromium/Electron with the X11 Ozone backend, or GTK with `GDK_BACKEND=x11`) needs a real app.

### 5.4 Real-app tests still needed before trusting it

The fixture answers every request (at the target or at its minimum) and is a winit client; the 6.14 simulation names the gaps. Needed, per app, 20+ runs idle and loaded, measured by the existing extension-only signal plus a screen capture for the cases the client cannot self-report:

| App | Toolkit / kind | Why |
| :--- | :--- | :--- |
| Firefox (snap) | GTK3, native Wayland, slow starter | flashed 6/6 in 6.12-6.13; its first answer is 135-312 ms late |
| gnome-text-editor, Nautilus | GTK4, remembers its size | min-bound (30x20 request); persistence side effects of the probe |
| gnome-terminal | GTK3, size increments | `AdjustedTo{Increment}` |
| a Qt6 app (e.g. KDE app) | Qt, native Wayland | different configure/ack timing; not yet traced at all |
| Chrome / Electron (VS Code, Claude desktop) | Chromium, XWayland by default (UNVERIFIED) and native with the Ozone flag | the X11-kind path, sync single counter |
| GTK4 app with `GDK_BACKEND=x11` | GTK4 over XWayland | sync client in the X11-kind path (`thawed` question) |
| an SDL/Wine/Steam window | XWayland | no sync (3.2: Xt-style clients do not advertise it) |

On this X11 session: a GTK4 app (gnome-text-editor) versus the fixture through the same run, to test the 3.2 hypothesis that sync clients do not flash.

## 6. Smallest implementation slice for the 2.0 beta

Goal: fix the measured defect on GNOME 46 native Wayland (stale `SIZE_SETTLED`, 25-42% post-reveal corrections under load) without closing any door in sections 2-4.

| Step | Content | Touches | Compatibility |
| :---: | :--- | :--- | :--- |
| S1 | `spawn_at_core::ready_wait`: `Policy` (defaults above), `ResizeOutcome`, `WindowKind`, the pure state machine for EXACT/QUIET/hard (no PROBE yet) with a virtual clock; golden vectors | core, tests | none (unused) |
| S2 | extension: replace the internals of `waitForCommit` with the JS machine (same module for modern and legacy via injection); EXACT + first-different-answer-then-QUIET, cap 250 ms, hard 1500 ms; `allows_resize()` skip; classify by `get_client_type()` and **route `XWayland`/`X11` windows to the old path explicitly, logged as `READY_WAIT_START strategy=legacy-x11`**; emit `READY_WAIT_*` tags; keep `COMMIT_RESOLVED` | `commit.js`, `extension.js`, `ProtocolVersion` 6 | policy args ignored by old extensions; old reasons still logged |
| S3 | CLI: read the outcome from the `ResizeOutcome` signal when protocol >= 6 (see 2.3; not `SpawnClaimed.error`); print it in `--json` as `resize`; no exit-code change. *Wired without the `policy` args (the extension uses the JS defaults, which a test keeps equal to the Rust ones) and without the new stderr lines of 2.3* | `gnome/dbus.rs`, `gnome/mod.rs` (now `gnome/driver.rs`), `spawn.rs` | additive |
| S4 | verify: acceptance cells A1-A3, A6-A8 with the existing fixture plus `--resize-delay-ms` and `--stall-after-map-ms`; record `lead_ms` and cloak numbers per commit in TEST_PLAN | harness | none |
| S0 (parallel, tiny) | observe-only `READY_OBSERVE` logging of `damaged`/`thawed`/`first-frame`/`size-changed` for X11-kind windows | extension | log-only |

Waits (each is designed above, none blocks the slice): the PROBE and the learned minimum (need P2/P3 data; interface stays in the core); GNOME 50 (`shell-version`, `[ok,w,h]` handling, `get_min_size`, optional external constraints); X11 and XWayland strategies (need S0 data); the Rust-side X11 damage/sync listener (standalone `X11Driver`; also needs a hide step that does not exist); wlroots/KDE; Windows; macOS; a typed D-Bus signal; a user-facing `--strict-size`.

The slice does not close doors because: the contract (steps, `ResizeOutcome`, `Policy`, `WindowKind`) exists in the core before any second backend; X11-kind windows are routed by kind from the first commit, so adding their strategy later changes a match arm, not the flow; the outcome is already on the wire and in the JSON (additive), so growing it (new reasons, a typed signal) does not break readers; constants have one source.

### 6.1 Risks

1. **Two implementations of one machine** (Rust and JS). Mitigation: shared golden vectors, one module for both extension trees. Residual: behaviour that only shows against real Mutter.
2. **Longer cloak changes app timing** (6.14 "feedback"; 6.13: callbacks to a cloaked surface wait for the next stage update), so the simulation may overstate the gain. Mitigation: A2/A3 measured on the real code (P2), not the simulation.
3. **First-spawn cost for refusing apps**: up to the cap (250 ms) against 120 ms today when the minimum is unknown and no probe/cache exists (D2).
4. **Signals not understood** (`damaged`, `thawed`): the X11-kind strategies stand on UNVERIFIED semantics. Mitigation: S0 observes before anyone waits on them.
5. **The fixture is not representative on X11** (no sync client) and has never been run as XWayland. Mitigation: 5.4.
6. **Probe side effects** (extra resize; persisted sizes in apps that remember them). Mitigation: off by default, decided in P3.
7. **Learned-minimum poisoning** by transient states (maximised, tiled, monitor edge). Mitigation: only the probe or an API/hint creates entries; contradiction deletes.
8. **GNOME 50 / API drift**: `get_min_size` return convention and `shell-version`; mutter 50 may drop the X11 backend (UNVERIFIED (web)). Mitigation: feature detection by presence; `[ok,w,h]` normaliser before any 50 claim.
9. **Frame vs buffer rectangle** on X11 and client-decorated GTK (shadows of 55-67 px, `_GTK_FRAME_EXTENTS`): comparing the wrong rectangle makes `Reached` impossible. Mitigation: the machine compares the same rectangle the request used (`move_resize_frame` takes the frame rect), logs both.
10. **Protocol skew** (new CLI with an old extension and the reverse). Mitigation: additive args, version gate, no new instruction names.
11. **Visibility is not the metric**: a numeric flash under one frame may not be seen (6.16); acceptance uses the numeric count so a faster or slower screen does not move the bar.

### 6.2 Decisions the user must make

| # | Decision | Recommendation |
| :---: | :--- | :--- |
| D1 | Cap `cap_ms` (100 / 150 / 250) and keep `hard_ms` 1500 | 150 and 1500 at first (6.14: 102 of 134 loaded flashes avoided at +33.5 ms mean, 0 idle cost); **250 and 1500 since section 11** |
| D2 | Accept up to +30 ms on the first spawn of a refusing app until a minimum is learned | yes for the beta; revisit with the cache |
| D3 | Probe in the beta | no; prototype (P2/P3) first |
| D4 | Learned-minimum store: none in the beta; later memory in the extension, file for non-resident backends | as stated in 4.4 |
| D5 | Outcome transport: overload `SpawnClaimed.error` on success + `ProtocolVersion` 6 now, typed signal in Stage 4 | **Changed at wiring time: the typed `ResizeOutcome` signal with `ProtocolVersion` 6 now, no overload** (section 2.3). A new signal does not force both sides to change together: an old CLI never subscribes, an old extension never emits |
| D6 | How much to invest in GNOME-on-Xorg given the (UNVERIFIED) removal of the X11 backend in mutter 50, while XWayland stays | observe-only now; build a wait only for X11-kind windows, share it between Xorg and XWayland; no Xorg-only work |
| D7 | Remove the CLI-side `settle_window_geometry` once the outcome arrives, or keep it as a fallback for protocol < 6 | keep as the fallback, read `Policy` for its constants |
| D8 | Allow harness-only fixture flags (`--resize-delay-ms`, `--max-width/--max-height`, `--resize-increment`, `--stall-after-map-ms`) | yes (harness only, no product code) |
| D9 | Adopt the acceptance table in 5.2 as the gate, including the sample size (n = 100 per cell) | yes; it costs about 15 minutes per cell set |
| D10 | Ship GNOME 50 support in the same beta or after | after; first fix `_getMinSize` and the manifest |
| D11 | A user-facing exit code for `TimedOut` (`--strict-size`) | not in the beta |

## 7. Evidence log of this session

Read-only commands only; nothing was written to settings, `/sys`, `/proc`, or dconf. Probe applications (fixture, xterm, xclock, xcalc, gnome-text-editor, gnome-calculator) were started for about 2 s and closed (no process left; `pgrep` empty afterwards).

- `xprop -root _NET_CLIENT_LIST`, `xprop -id <id> WM_CLASS WM_PROTOCOLS _NET_WM_SYNC_REQUEST_COUNTER WM_NORMAL_HINTS _GTK_FRAME_EXTENTS` for the windows in the table in 3.2; `xprop -root _NET_SUPPORTED` (79 atoms; `_NET_WM_FRAME_DRAWN`, `_NET_WM_FRAME_TIMINGS` present; `_NET_WM_SYNC_REQUEST` is a client protocol, not a root atom).
- `gjs` introspection of `/usr/lib/x86_64-linux-gnu/mutter-14/Meta-14.typelib` (a throwaway script in `scratch/`, gitignored, run with `GI_TYPELIB_PATH` and `LD_LIBRARY_PATH` set for that one process): `WindowActor` signals `first-frame, effects-completed, damaged, thawed`; `Window` has `get_client_type, allows_resize, allows_move, get_buffer_rect, get_frame_rect, get_frame_type, is_client_decorated, get_pid, get_sandboxed_app_id, get_gtk_application_id, move_resize_frame`; no `get_min_size`, `get_max_size`, `get_client_pid`; `WindowClientType {WAYLAND:0, X11:1}`; `ShapedTexture` signal `size-changed`.
- `strings`/`nm -D` on `libmutter-14.so.0` (46.2): sync-request, freeze/thaw and XWayland-thaw symbols listed in 3.2.
- Vendored sources: `x11rb 0.14.0` has `damage` and `sync` features; `winit 0.29.15` contains neither `_NET_WM_SYNC_REQUEST` handling nor `WINIT_UNIX_BACKEND`.
- Repository reads: the files and lines in section 1; `verify-results/` protocol census (local, gitignored, not in the repository) (`proto=Wayland` 230 732, `proto=X11` 5 132, no `XWayland`).
- Unverified leads, summarised by a small model and therefore not relied on: mutter NEWS on the X11 backend (49.alpha.0, 50.alpha) and the 6.14 summaries for GNOME 50 APIs.

## 8. Status: core machine, golden scripts and JS port (2026-10-13)

Implemented exactly the contract of 2.2 and 4.3 for the native Wayland strategy, with nothing wired into the extension, the CLI, JSON or D-Bus. Probe, learned-minimum cache and X11/XWayland/other-OS strategies are not implemented.

| Piece | Where | Commit |
| :--- | :--- | :--- |
| Rust machine: `Policy` (250 / 30 / 1500 ms), `Request`, `Outcome`, `Action`, `ReadyWait::{start, on_sample, on_tick}`; no I/O, time passed in as milliseconds | `crates/spawn-at-core/src/ready_wait.rs` (9 unit tests) | `50b0002` |
| 20 language-neutral golden scripts, the virtual-clock driver (README), shared `policy.json` | `tests/ready_wait_vectors/`; Rust runner `crates/spawn-at-core/tests/ready_wait_vectors.rs` | `e9e2265` |
| JS port: no imports, no host APIs (a test asserts it) | `assets/gnome/modern/ready_wait.js`; `tests/ready_wait_test.js` runs the same 20 scripts | `9f78a1d` |
| Installer list entry | `src/platform/linux/gnome/deploy.rs` (it was `gnome/mod.rs` before the step 3 split) | `902dff3` |

**Differences from the text above, decided while implementing (CONFIRMED (source)):**

1. *Ties and late input* were unspecified. Rule, pinned by scripts 04, 10c, 14 and unit tests: a sample on a deadline beats the timer; a sample after a deadline is handled as if the timer had fired on time and the outcome reports the deadline, not the late arrival; hard beats quiet when equal.
2. *The machine API* is `start` / `on_sample` / `on_tick` returning `Action::{Wait{deadline_ms}, Done(Outcome)}` instead of `step(state, event, now)`; same information.
3. *Outcome and Policy are the subset* described in the two notes in 2.2 and 4.2.
4. *Hard limit in the quiet phase* returns `TimedOut{Hard}` with the last size, as 4.3 says, although the window had answered (scripts 13 and 14). A caller that prefers `AdjustedTo` there can decide that when it is wired; the rule is one line in `expire`.
5. **The installer ships the file.** I expected the unwired module to stay out of the installer; it does not: `test_embedded_extension_files_match_disk` requires the embedded list to equal `assets/gnome/modern`, so `ready_wait.js` is listed (and copied to the user's extension directory) but nothing imports it. Inert at runtime; it is the one change outside the allowed files, forced by an existing guard. Revert = remove the list entry together with the file.
6. *The legacy tree (42-44)* has no copy. The module is ES-module syntax for `modern`; the legacy tree needs the same logic in its `var`/`require` style, which belongs to the wiring step (4.1 "plain JS module imported by each tree").

**Evidence the two builds agree:** both run the 20 scripts, with expectations derived by hand from the rules before either ran them (they matched on the first run of the Rust machine; the JS port also matched on its first run). Mutation check of the JS port (temporary, restored, file byte-identical): changing the late-input comparison `>` to `>=` fails scripts 04 and 10c; changing the hard-limit test `>=` to `>` fails 12, 13, 14 and two mirrored unit cases. Counts: core 34 (was 23), node 101 (was 70); the verify counts did not change (352 / 338 / 236).

**Not covered by these tests:** that the real extension feeds the machine the right samples (`size-changed`, not `allocation`), a real Mutter, and any app other than the scripted ones. Those are the wiring step's tests (section 5).

## 9. Status: wired into the modern extension, the CLI and the JSON (2026-10-13)

Labels: CONFIRMED = read in the code or run in a test listed here; HYPOTHESIS = not run. **Nothing in this section was run in a GNOME Shell.** The extension changes take effect only after the user reinstalls the extension and logs out and in; real Mutter sample timing is unmeasured.

| Step | Commit | What |
| :--- | :--- | :--- |
| 1 | `94d9c3c` | `commit.js` `CommitLatch.waitForReady` runs `ReadyWait` on the window **frame** size (one read at the request plus one per `size-changed`; `notify::allocation` is not connected), the helper's `nowMs()` clock and timers (one for the machine's deadline, one for the hard limit). `extension.js` `_stepWaitForCommit` picks the strategy per window: native Wayland (`get_client_type()` is `WAYLAND`) with a size request uses the machine, target = the requested size after the existing floor clamp (`ctx.safeW/safeH`); X11, XWayland, unknown, and a request without a size run `waitForCommit` unchanged and log `strategy=legacy`. Skipped when `allows_resize()` is false (feature-detected with `typeof`; a window without the method counts as resizable), fullscreen, or tiled. The cloak watchdog (6000 ms), the late-resize pass and the one-second corrector are untouched |
| 2a | `5ad4023` | `ProtocolVersion` 6; D-Bus signal `ResizeOutcome(target_id s, outcome s, final_w u, final_h u, waited_ms u, reason s)`, emitted in `_reveal` just before `SpawnClaimed`, only when the machine ran |
| 2b | `082b1f0` | CLI subscribes to `ResizeOutcome` when the extension reports protocol >= 6 and attaches the newest outcome for the claimed target to `ClaimResult.resize`; `--json` gains `resize` (`spawn-at-core/src/resize_report.rs`). Exit codes and stderr text unchanged |
| 2c | `65c011b` | section 2.3, D5 and S3 of this document |
| 3 | `993e673`, `1a255f4` | Rust and node tests |

**Log tags (CONFIRMED, `tests/ready_wait_wiring_test.js`).** `READY_WAIT_START strategy=ready-wait kind=NativeWayland target=(WxH) current=(WxH) cap=250 quiet=30 hard=1500`; `READY_WAIT_RESOLVED strategy=ready-wait outcome=<reached|adjusted|timed-out|skipped> elapsed_ms=.. waited_ms=.. reason=<at-target|quiet|cap|hard|not-resizable|fullscreen|tiled> finalFrame=.. target=..`. Legacy: `READY_WAIT_START strategy=legacy kind=<X11|XWayland|NativeWayland|unknown> why=<not-native-wayland|no-size-request> timeoutMs=..` and `READY_WAIT_RESOLVED strategy=legacy outcome=none elapsed_ms=.. reason=<AT_TARGET|SIZE_SETTLED|TIMEOUT_NO_CHANGE|HARD_CAP>`. `COMMIT_SAMPLE` is still written (sources `initial` and `size-changed` on the new path) and `COMMIT_RESOLVED` is still written once per wait, with `reason` `AT_TARGET`, `SIZE_SETTLED`, `TIMEOUT_NO_CHANGE`, `HARD_CAP`, and `SKIPPED` for a skipped wait (new). The verify analyser reads only the tag names (`CLOAK`, `COMMIT_RESOLVED`, ...), so it was not changed (CONFIRMED (source), `trace/phases.rs`, `trace/analyser.rs`).

**Differences from the design text (CONFIRMED (source)).**
1. The strategy tag is `strategy=legacy` for all non-native-Wayland windows, not `legacy-x11` (S2); the window kind is a separate `kind=` field.
2. Transport: a typed `ResizeOutcome` signal instead of the `SpawnClaimed.error` overload (section 2.3, D5).
3. No `policy` object is sent in the `WaitForCommit` args and the extension uses `DEFAULT_POLICY` (250 / 30 / 1500) (section 10 adds an optional `cap_ms` for a developer override, absent by default); the shared `policy.json` test keeps the JS and Rust defaults equal. The `timeout_ms` argument is ignored on the new path (still read by the legacy path).
4. No new stderr lines (section 2.3 allowed them); only the JSON `resize` object is new. `settle_window_geometry` on the CLI side is unchanged (D7: kept).
5. A skipped wait reports the frame size at the end of the wait as `final` and `waited_ms` 0.
6. The legacy extension tree (GNOME 42-44) has no wiring and stays at protocol 5. Because the modern D-Bus XML now has a signal the legacy XML lacks, both XML parity tests (`legacy_dispatcher_test.js`, `gnome/tests.rs`, before step 3 `gnome/mod.rs`) compare the legacy XML with the modern XML minus `ResizeOutcome`. The legacy tree is byte-identical to before.
7. `outcome.waited_ms` in the signal is the machine's number (the deadline that fired), not the wall time; `elapsed_ms` in the log is wall time on `nowMs()`.

**Tests (CONFIRMED, run).**
- Node, `tests/ready_wait_wiring_test.js` (33): the 20 golden scripts through `CommitLatch.waitForReady` on a virtual clock (outcomes equal the scripts' `expect`; no handler survives; a skipped or already-satisfied request connects none); 13 scenarios through the real `_runBatch` with a mock window: fast (reached at 20 ms), slow (220 ms), refusing (TimedOut at 250 ms, placed at 400x300), adjusted (494x292, quiet, placed for 494x292), restless (hard limit at 1500 ms and the batch still reveals), not resizable, fullscreen and tiled (Skipped), allocation not connected, feature detection by presence, X11 and XWayland (legacy, no `ResizeOutcome`, allocation still watched), no size request, unknown window kind. The signal order `ResizeOutcome` before `SpawnClaimed` is asserted.
- Mutation check (temporary, restored): routing every window to the machine fails 3 tests; not disconnecting `size-changed` fails 16.
- Rust: 4 `resize_report` unit tests; 3 `--json` tests (shape, no `resize` key when absent, older output parses); 3 claim tests against stub services on the real session bus with two connections (protocol 6 attaches the outcome and ignores another target's; protocol 6 without an outcome returns at once; protocol 5 has none). The protocol 6 claim test passed 40 of 40 repeated runs (ordering of the two signals across two streams on an idle machine).

**Not verified.**
- Real Mutter: whether `size-changed` on a real native Wayland window arrives with the frame size the app committed, and how late under load. The sample source was changed from the allocation to the frame size on the strength of TEST_PLAN 6.12; the effect on the 15 of 20 flashing runs at 22 threads (6.17) is **unmeasured**.
- Tiled detection: `get_tile_match` is in the Meta-14 typelib (CONFIRMED by `strings`), but whether it is non-null for a window tiled alone, and whether `tile_mode` exists, was not run (HYPOTHESIS). A tiled window that is not detected just waits to the cap.
- `allows_resize()` on the fixture and on real apps; GNOME 45, 47, 48; the legacy tree.
- That the CLI sees `ResizeOutcome` before the claim when GNOME Shell is the sender and the machine is loaded (HYPOTHESIS; the stub test covers ordering on an idle machine only).
- The cloak now stays up to 250 ms longer for a refusing app and up to 1500 ms for a restless one (risk 3 of 6.1); its effect on the cloak numbers was not measured.

## 10. Per-spawn override for same-login A/B (2026-10-13)

Labels: CONFIRMED = read in the code or run in a test listed here; HYPOTHESIS = not run. **Nothing in this section was run in a GNOME Shell**, and no live session was touched while writing it (node and cargo tests only). The extension change takes effect only after the user backs up and reinstalls the extension and logs out and in.

**Why.** TEST_PLAN 6.18 compared the change across two logins, and the legacy XWayland path, which the change does not touch (CONFIRMED by diff), moved from 1 of 10 to 9 and 10 of 10 flashes between them. A before/after across logins cannot separate the change from the login. The override lets one login run both rules, arm against arm, interleaved.

**Interface (CONFIRMED, tests below).**

| Where | Option | Values | Visible in `--help` |
| :--- | :--- | :--- | :--- |
| `spawn-at spawn` | `--wait-strategy` | `auto` (same as absent), `legacy`, `ready-wait` | no (`hide = true`) |
| `spawn-at spawn` | `--wait-cap-ms` | whole milliseconds, 1 to 1500 (the hard limit of `Policy`) | no |
| `spawn-at-verify run` | `--wait-strategy`, `--wait-cap-ms` | passed as typed to every `spawn` action | **yes**, in `run --help` (a decision of this step: `run` is the A/B driver and a developer tool; say so if it should be hidden too) |

**Carrier and protocol (CONFIRMED).** The override travels in the existing `ArmSpawn` instruction JSON as two optional fields of the `WaitForCommit` instruction: `{"WaitForCommit":{"timeout_ms":300,"strategy":"legacy","cap_ms":200}}`. The D-Bus interface and `ProtocolVersion` are **unchanged (6)**; no protocol bump is needed. The fields are skipped when unset (`skip_serializing_if`), so a spawn without the flags sends the bytes it always sent. They live in the arguments of that one instruction in that one `ArmSpawn`, so they apply to that spawn only; nothing is stored in the extension. Consequence for mixed versions: an extension without this change (including the protocol 6 build of section 9) ignores the two fields without error, so an A/B run against it is two default runs. The harness makes that visible (below) instead of the CLI refusing, because the CLI cannot tell the two protocol 6 builds apart.

**What each value does (CONFIRMED, `tests/ready_wait_wiring_test.js` section 3).**

| `--wait-strategy` | native Wayland window with a size request | any other window (X11, XWayland, unknown, no size request) |
| :--- | :--- | :--- |
| absent / `auto` | the machine (cap 250 unless `--wait-cap-ms`) | original rule, `why=not-native-wayland` / `no-size-request` |
| `legacy` | **original 30 ms rule** (`waitForCommit`, idle timeout 300 ms): `READY_WAIT_START strategy=legacy kind=NativeWayland why=override-legacy timeoutMs=300`, no `ResizeOutcome` signal | original rule, as under `auto` |
| `ready-wait` | the machine, as under `auto` | original rule, as under `auto`, reported as unused. The machine is not forced onto X11 or XWayland: it would read their immediate frame rect as an instant answer (section 3.1) |

`--wait-cap-ms N` replaces the cap of the machine (`request.policy = {cap_ms: N}`, merged over `DEFAULT_POLICY`); with `legacy`, or on a window that takes the original rule, it has nothing to apply to and is reported as unused.

**Invalid values (CONFIRMED).** Two layers, both ignore and say so. `spawn-at` keeps the two options as text so clap cannot fail the spawn; `WaitOverride::from_raw` (core) drops a bad value, keeps the other one, and the CLI prints one `WARNING` per dropped value (`ignoring --wait-strategy 'fast' (expected auto, legacy or ready-wait)`). A harness spec that forbids `[spawn-at] WARNING:` in stderr therefore fails on a typo instead of silently running the default arm. The extension validator (`validator.js`) repeats the check for instructions that did not come from this CLI: a bad `strategy` or `cap_ms` (not an integer, outside 1 to 1500, any other type) is dropped, never thrown, and named in `READY_WAIT_OVERRIDE ... ignored=` (value text cut to 32 characters). An invalid `timeout_ms` still rejects the instruction, as before.

**Log lines (CONFIRMED).**
- `READY_WAIT_OVERRIDE strategy=<legacy|ready-wait|none> cap_ms=<N|none> unused=<field(reason);..|none> ignored=<field=value;..|none>`: written once, before `READY_WAIT_START`, **only when the instruction carries an override field**. A default spawn writes no such line.
- `READY_WAIT_START` shows the effective values: `strategy=ready-wait ... cap=<effective>` for the machine, `strategy=legacy ... why=override-legacy` when the override sent a native window down the original rule. Every `READY_WAIT_START` of a spawn without the flags is byte-for-byte what section 9 logged (all 134 earlier node tests, which assert the exact lines, pass unchanged).

**Defaults do not change (CONFIRMED by tests).**
- Rust: the two existing golden tests that assert the exact `WaitForCommit` string (`mechanics.rs`, resize-only and anchored resize) pass unchanged (their lines are not in the diff); a new test builds a resize entry with the default override and with `--wait-strategy auto`, and asserts the exact string `["Cloak",{"SetSize":{"w":500,"h":400}},{"WaitForCommit":{"timeout_ms":300}},"Uncloak"]` for both; `run_spawn` with no flags arms an entry whose `wait` is the default and serialises without a `wait` key.
- Node: the 134 earlier tests are unchanged and pass (the wiring test helper `runBatch` was split into `makeRig`/`runOn`/`runBatch` to let one extension instance run two batches; its callers are unchanged); a new test runs the same app with no field and with `strategy:'auto'` and asserts identical log lines, identical signals and the same final frame.
- The `ready_wait_vectors` golden scripts (Rust and JS) are untouched and pass.

**Harness (CONFIRMED, tests).** `run --wait-strategy X --wait-cap-ms N` adds the two options to every `spawn` action, after the spec's own action arguments and before the `--` that starts the fixture; not to other commands; not again if the spec already spells that flag out; unchanged command line without the options. Each result in `report.json` gains, next to the flash keys: `wait_strategy` (`ready-wait` or `legacy`, from the window's `READY_WAIT_START`), `wait_cap_ms` (the logged `cap=`, only for `ready-wait`), `wait_requested` (`{strategy, cap_ms}` as typed, only when `run` was given an option) and `wait_notes` (an override with no effect, values the extension dropped, or "override requested but the extension logged no READY_WAIT_OVERRIDE"). A run with no trace has none of the keys; an old report reads back as nothing recorded. The record comes from the extension's log, not from what was asked, so an arm that did not take effect is visible per run.

**A/B recipe (HYPOTHESIS: not run).** `scratch/ab_batch.sh <out> <n> <bin> "legacy:--wait-strategy legacy" "rw:--wait-strategy ready-wait" -- <spec>...` interleaves the arms with the arm order rotating each iteration (the script is throwaway and not committed). Before reading any number, check per arm that `wait_strategy` is what the arm asked for and `wait_notes` is empty.

**Tests added (CONFIRMED, run; counts in TEST_PLAN 1.1).** Core 9 (`wait_override.rs`); `spawn-at` 11 (4 CLI, 3 `run_spawn`, 4 instruction); node 20 (9 validator, 11 wiring); verify 16 (9 `trace/wait.rs`, 4 argument builder, 1 report JSON, 2 `run --help` and parse).

**Not verified.**
- Everything in a real GNOME Shell: that the installed extension logs `READY_WAIT_OVERRIDE` and takes the original rule for a native Wayland window under `legacy`; that `legacy` on native Wayland behaves like the pre-wiring build (the code path is the old `waitForCommit` call, but the old build was not run side by side).
- That the legacy rule on a native Wayland window gives the results 6.17 measured before the wiring (same code, different login: exactly the gap this switch is for).
- The 42-44 extension tree: it has no ready-wait and drops the fields (CONFIRMED by a validator test), so it needs no switch.
- `transform` and the X11 driver take no override (`WaitForCommit` in `transform` is built with the default).
- Whether `ready-wait` should force the machine on XWayland for a deliberate experiment: not done (it would produce a meaningless instant answer).

## 11. Default cap raised from 150 ms to 250 ms

**Change (CONFIRMED, code and tests).** `Policy::default()` (Rust), `DEFAULT_POLICY` (JS) and `tests/ready_wait_vectors/policy.json` now read `cap_ms` 250. `quiet_ms` 30 and `hard_ms` 1500 are unchanged. The golden vectors 02 to 05 moved with the cap (02: answer at 240 ms is `reached`; 03: answer at 300 ms is `timed-out` at 250 ms; 04: answer exactly on 250 ms is `reached`; 05: refusing app times out at 250 ms); 12 sets its own cap (5000) and 13 to 15 do not depend on it. Rust core tests, node tests and the shared-file equality test pass (counts in TEST_PLAN 1.1). A developer can still pass `--wait-cap-ms` (section 10); the field and its 1 to 1500 range are unchanged.

**Why (evidence is in TEST_PLAN, not here).** 6.19: same-session loaded A/B/C, cap 250 avoided 76.6% of the flash runs of the legacy rule where cap 150 avoided 68.1%. 6.20: all 5 loaded flashes of the 150 ms arm that were not a late first-paint stamp ended at the cap (`timed-out`), 4 of them for the larger spec; the 250 ms arm had none.

**Cost (not measured on a real refusing app).** An app that never answers a size request (it is at its minimum and the minimum is unknown) now holds the cloak up to 250 ms instead of up to 150 ms on its first spawn (the risk noted in section 9, acceptance A4 is written against the old cap and has not been re-run). 6.19 loaded cloak numbers for the two caps, same session: B (150) median/p95/max 131.6/167.0/183.9 ms, C (250) 129.3/200.9/263.4 ms. A hung app is still ended by the hard limit at 1500 ms.

**Installed extension.** The change is in the repository tree. An extension already installed in a login session keeps the cap it was installed with until it is reinstalled and the session restarts; a run that wants 250 ms on such an extension passes `--wait-cap-ms 250`.

## 12. X11 observation (observe-only logging, 2026-10-13)

Labels: CONFIRMED = covered by a test in this tree (node, fake host); HYPOTHESIS = needs a real Shell. The first live runs are in TEST_PLAN 6.23 to 6.26.

**Why.** On X11 and XWayland the window manager sets the frame size at once, so `frame == target` says nothing about whether the app has painted into it (TEST_PLAN 6.15, 6.18); that is why those windows keep the original quiet rule (`strategy=legacy kind=XWayland|X11`). The ready-wait machine needs a signal that the app has finished painting a resize. Candidates from the Mutter documentation: `Meta.WindowActor::damaged` ("one or more surfaces damaged"), `::first-frame` (first frame only), and the size of the actor's surface texture, which may follow the committed buffer on XWayland. None of it is verified for GNOME 46. This section only **looks**; no strategy, verdict, timing or outcome uses it.

**What is logged (modern tree only; `assets/gnome/modern/x11_observe.js`).** For a spawned window whose client type is X11 (XWayland under a Wayland session, or native X11), from the start of its batch until 500 ms after the reveal, only while the `SetLogging` flag is on. All lines go through the existing timing log (`[spawn-at-time] +N.NNms | TAG win=[...] ...`), so they land in a run's `log_slice` unchanged (the journal filter keeps every `[spawn-at-time]` line; test in `log_source.rs`). `mono_ms` is `GLib.get_monotonic_time()/1000`, the clock of the extension's other timestamps and, after the harness's clock anchor, of the fixture's events.

| Line | Fields | When |
| :--- | :--- | :--- |
| `X11_OBS_START` | `kind` (`XWayland` or `X11`), `wayland` (compositor is Wayland), `client_type`, `id`, `mono_ms`, `damaged=available\|unavailable`, `first_frame=available\|unavailable` | start of the batch |
| `X11_OBS_NOTE` | `damaged unavailable` | once per extension run, if `damaged` is not a signal of the actor class |
| `X11_OBS_MARK` | `what=set_size\|commit_resolved\|reveal`, `id`, `mono_ms` | after the resize request, after the wait resolves (either rule), at the reveal |
| `X11_OBS_SIGNAL` | `signal=damaged\|first-frame`, `id`, `mono_ms`, `seq`, `texture=(WxH)`, `actor=(WxH)`, `frame=(x,y,WxH)`, `buffer=(x,y,WxH)` (`(na)` when a value cannot be read) | each emission |
| `X11_OBS_END` | `id`, `reason=unmanaged\|reveal+500ms\|disabled\|error`, `mono_ms`, `damaged=N`, `first_frame=N` | end of the watch |

`damaged` and `first-frame` are looked up with `GObject.signal_lookup` on the actor's class at run time, never by version number. The existing `SET_SIZE_REQUEST`, `READY_WAIT_*`, `COMMIT_RESOLVED` and `UNCLOAK_4D_REVEAL` lines are still written, so the marks can also be joined on their `+N ms` stamps.

**Observe-only guarantees (CONFIRMED by tests, `tests/x11_observe_test.js` and the last block of `tests/ready_wait_wiring_test.js`).**
- With logging off the observer asks only "is logging on?": no signal handler on the actor or the window, no timer, no feature lookup, no line (tests: fake host and the extension with an X11-kind window).
- Handlers are disconnected when the window is unmanaged, 500 ms after the reveal, or when the extension is disabled; afterwards the actor has no handler left from it and a late emission logs nothing.
- Native Wayland windows are never watched. A host call that throws never reaches the compositor's signal emission.
- With logging on, the window ends at the same frame with the same wait and reveal lines as with logging off (the only extra lines are the `X11_OBS_*` ones and the actor traces, which have always been written only when logging is on).
- The harness changes nothing: the lines are plain text in `log_slice`, and the flash result with them in the slice equals the one without (test in `runner/wire_paint_tests.rs`).

**Open questions this will answer (HYPOTHESIS until a Shell run).**
1. Does `damaged` exist on `Meta.WindowActor` in GNOME 46? (`damaged=available` or the note.)
2. On XWayland, when does the texture size change relative to `set_size` and to the app's repaint: is there a `damaged` after the texture reaches the target size, and is it distinguishable from the damage of the resize itself?
3. Does the texture or buffer rect ever differ from the frame rect after `set_size`, i.e. can "texture == target" stand in for "the app painted at the target size"?
4. How long after `set_size` does the last `damaged` of a resize arrive, compared with `commit_resolved` (the legacy quiet rule's answer) and with the fixture's `FIRST_PAINT` / wire commit (TEST_PLAN 6.21)? That is the gap a better rule would close.
5. Is `first-frame` emitted for a window that is already mapped and resized (expected: only once, at map)?

**Not shown.** Everything about real emissions: that the signals fire, their order and number, whether `get_texture().get_width()` is the surface size, and whether a handler on `damaged` costs measurable time in the compositor (it only formats a line when logging is on, but the handler is connected whenever logging is on).

**Update 2026-10-13: first live run, and the texture reader corrected.** Labels: CONFIRMED = seen in a run or in the typelib in this tree; HYPOTHESIS = not run in a Shell.

- First live run (TEST_PLAN 6.23, 80 XWayland fixture runs): questions 1 and 5 are answered. `damaged` exists (`damaged=available` in 80 of 80 runs) and `first-frame` came once per run, before the client's resize and paint. The first `damaged` after `set_size` does not follow the client's paint (6.23, "Candidate rules").
- **Why `texture=(na)` in 351 of 351 lines (CONFIRMED by introspection).** Read with standalone `gjs` outside the Shell (`GI_TYPELIB_PATH=/usr/lib/x86_64-linux-gnu/mutter-14`, Meta-14 of GNOME 46; the script was a throwaway in `scratch/`):

  | Class | Has | Does not have |
  | :-- | :-- | :-- |
  | `Meta.WindowActor` | `get_texture()` (returns `Meta.ShapedTexture`, may be null), `get_image`, `paint_to_content`; from `Clutter.Actor`: `get_width`, `get_height`, `get_size`, `get_content`, `has_damage`; signals `first-frame`, `damaged`, `effects-completed`, `thawed` | |
  | `Meta.ShapedTexture` (a `GObject.Object`, not an actor) | `get_texture()` (returns `Meta.MultiTexture`, never null), `get_image`, `set_mask_texture`, `set_create_mipmaps` | **`get_width`, `get_height`** |
  | `Meta.MultiTexture` | `get_width()`, `get_height()` (int), `get_n_planes()`, `get_plane(n)` (returns `Cogl.Texture`), `get_format()`, `is_simple()` | |
  | `Cogl.Texture` | `get_width()`, `get_height()` | |
  | `Meta.SurfaceActor` | not in the typelib | |

  So the old reader called `get_texture()` successfully and then `get_width?.()` on a `ShapedTexture` that has none: `undefined`, hence `(na)`. My earlier guess that the method was missing from the window actor (TEST_PLAN 6.23, written before this) was wrong.
- **New reader (`readTextureSize` in `x11_observe.js`, tested in Node with mocks of this shape).** It tries `shaped>multi` (`actor.get_texture().get_texture().get_width()/get_height()`), then `shaped>multi>plane0` (`...get_plane(0)`), each in its own try/catch. A log line now says `texture=(WxH) texture_via=<accessor>`, or `texture=(na) texture_err=<accessor>:<message>|<accessor>:<message>` with the real error text, so a failure can no longer look like "nothing". Still observe-only: only getters, no signal, timer or state added.
- **HYPOTHESIS, not run in a Shell:** that the accessor works on a window actor in the live Shell (the chain is from the typelib, not from a live object), and that the multi-texture size follows the client's committed buffer on XWayland rather than the frame. The first live run with the new reader answers both.

**X11/XWayland: investigated, no reliable signal (2026-10-13; TEST_PLAN 6.23 and 6.24).** Labels: CONFIRMED = measured on the fixture (`--protocol x11`, 160 runs idle and under load); HYPOTHESIS = not tested. The ready-wait stays Wayland-only; XWayland and X11 windows keep the legacy quiet rule.

| Candidate | Result | Why it fails (CONFIRMED) |
| :-- | :-- | :-- |
| First `damaged` after `set_size` | before the client's final paint in 59 of 80 runs (idle larger 20 of 20, 13 to 20 ms early) | it comes about 5 ms after `set_size` in idle and tracks the shell (the extension's own move to the final position, HYPOTHESIS), not the client |
| `first-frame` | one per run, 0.2 to 1.9 ms before `COMMIT_RESOLVED` | before the client's resize and paint in 80 of 80 runs |
| Actor size | equals the frame size in 351 of 351 lines | the window manager's size, set at once |
| Buffer rect | equals the frame rect in 351 of 351 lines | same |
| Texture size (`shaped>multi`) | equals the target at the first `damaged` in 80 of 80 runs; 400x300 at `first-frame` | before the client painted in 59 of 80 runs, up to 359 ms early under load; the same moment as the first `damaged` |

All of them fire at the shell-side resize, not at the client's paint. The client's own paint produces a `damaged` only 0.2 to 20 ms after the paint, and in the runs where the window was still cloaked it did not arrive before the reveal (0 of 14 idle larger runs). (Section 13 later showed that a cloaked window does get `damaged`; those events are the shell's, not the client's.)

**Further attempts (2026-10-14; TEST_PLAN 6.25 to 6.29; code removed, not in this repository).** Labels: CONFIRMED = measured on the fixture.
- **Hold the cloak and wait for a second `damaged`:** tried, **not usable** (CONFIRMED, 160 runs, 6.26). A cloaked window does get `damaged`, but the second one arrives about 30 ms after `set_size` whatever the client does (before the client's paint in 30 of 39 loaded runs); it cuts some flashes only by holding the window longer (+13 to +38 ms at the median).
- **Pre-map resize** (resize at `window-created`, reveal on `first-frame`): tried, **not usable** (CONFIRMED, 6.28; the early resize itself works). The app's first paint was at the target size in 6 of 10 idle runs, but `first-frame` is a map event and arrived 205 to 300 ms before a loaded app had painted (loaded `larger`: 5 of 5 flash, as with the legacy rule).
- **Window ping:** no API exposes the reply (CONFIRMED, 6.25): `Meta.Window.check_alive(timestamp)` is an action with no reply signal; the only observable is the read-only `is-alive` property.
- **Pixel probe** (sample a few pixels of the window texture through `WindowActor.get_image`): on the fixture it read a blank, fully transparent buffer right after the map and the fixture's green after its paint, at about 2.5 ms per probe (CONFIRMED, 3 smoke runs, 6.29), but in the first batch `get_image` returned a NULL surface, GJS aborts on that with an uncatchable exception, and GNOME Shell crashed (signal 5, 09:59:04, 6.29). It was therefore **removed**; no batch data exists, so whether blank-versus-painted is a usable ready signal is unknown.
- **Capture rig** (a screen-capture measurement of what is visible): the Mutter ScreenCast stream can be created without a prompt, but no local consumer could attach to its PipeWire node (6.25, 6.26) and the portal needs a user prompt; not built.

**Conclusion for X11 and XWayland.** The first `damaged`, `first-frame`, the actor size, the buffer rect, the texture size, the second-`damaged` hold and the pre-map resize all fire at the shell-side resize or at the map, not at the client's paint. XWayland and X11 windows keep the legacy rule. **Status after TEST_PLAN 6.31 to 6.35 (nothing of it ships in 0.2.0-beta.1, 0.2.0-beta.2, 0.2.0-beta.3 or 0.2.0-beta.4):** the X sync counter was tried with our own GTK4 client and is usable as an experimental signal for GTK4-style clients only (6.33: the extended counter steps a few ms after the client's final paint in 100 of 100 runs; the basic counter, which Chromium and Electron use, never moved; in the earlier survey 6 of 9 measured windows advertised some counter and about 3 of 9 would benefit); a sync request sent by us was negative at the timing tried (6.34; the extension that causes the resize cannot send X messages); an `LD_PRELOAD` geometry shim worked for GTK4 through Xlib on one machine (6.35: windows born at the target size in 80 of 80 runs, loaded flashes 0 of 40 against 36 of 40) but covers no xcb client, did not exempt override-redirect windows, left popups untested, reveals later, showed 10 of 40 idle stalls of unverified cause, and carries the security and compatibility concerns of injecting a library into apps. A screen-capture rig is still untried (needs a user prompt or a consumer that can attach to the Mutter node). Section 15 lists what a future per-app design could do.

## 13. Experiment: hold the cloak and wait for a second `damaged` (2026-10-14; removed)

Tried and not usable; the data is in TEST_PLAN 6.26 and the summary in section 12 above. The code (the module and its hidden strategy value) was removed after the experiments failed and is no longer in the tree (it is still in the git history of `main` up to commit `673e6f2`; an unpushed local branch `archive/x11-experiments` points at that commit).

## 14. Experiments: resize before the map, and a pixel probe (2026-10-14; removed)

Tried and not usable (pre-map resize, TEST_PLAN 6.27 and 6.28) or removed because it can crash the Shell (pixel probe, 6.28 and 6.29); summaries in section 12 above. The code (the two modules and their hidden strategy values) was removed and is no longer in the tree (it is still in the git history of `main` up to commit `673e6f2`; an unpushed local branch `archive/x11-experiments` points at that commit). The fixture's test-only `--state-colors` option stays (solid magenta at the initial size, green at any other).

## 15. Future direction (post-beta)

This is a direction, not a plan or a promise. Nothing here is implemented, and **none of it has been tested on real apps**: every measurement behind it used the project's own test window or our own GTK4 test client on one machine (TEST_PLAN 6.22 to 6.35). It needs real-app testing before any of it is relied on.

**Idea: pick the method per app type, with the legacy rule as the fallback.**

| App type | Method | Evidence so far |
| :-- | :-- | :-- |
| Native Wayland | the ready-wait (shipped in 0.2.0-beta.1) | TEST_PLAN 6.22 to 6.24 |
| GTK4 on X11 | read the extended sync counter (`_NET_WM_SYNC_REQUEST_COUNTER`) and reveal when it steps | 6.33: usable as an experimental signal for GTK4-style clients only |
| Apps with a launch option for size or position (`-geometry`, `--window-size`, ...) | pass the target at launch so there is nothing to resize | not tested |
| Apps that need it and accept the risks | an opt-in `LD_PRELOAD` geometry shim | 6.35: works for GTK4 through Xlib on one machine; real security and compatibility costs; not recommended to ship |
| Anything else | the legacy rule | shipped |

**Detecting the type** would use what is observable without running the app's code: the libraries it has loaded (for example `libgtk-4`, `libQt6Core`, an Electron or Chromium marker), the window class, and whether the window advertises `_NET_WM_SYNC_REQUEST_COUNTER` (and how many counters). **Per-app config overrides** would let the user force a method or turn one off for a class.

**Order after the beta:**
1. Real-app testing of what already ships, so later steps are judged against real behaviour.
2. **Opt-in native-Wayland launch env vars** (`GDK_BACKEND=wayland`, `QT_QPA_PLATFORM=wayland`, `MOZ_ENABLE_WAYLAND=1`, the Electron Ozone hint), so an app that can run on native Wayland gets the ready-wait instead of the XWayland legacy rule.
3. **Launch args per app** (a per-class table of arguments added to the command line). It is ahead of the shim: it changes no process internals and needs no injected code.
4. The GTK4-on-X11 counter as an experimental, opt-in signal.
5. The opt-in geometry shim, only if 1 to 4 leave a gap that matters, and with the security and compatibility concerns of TEST_PLAN 6.35 settled first.
6. **CI on pull requests** (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, tests). Done since this section was written: `.github/workflows/ci.yml` exists and runs these on pull requests (with `--workspace` and `--locked`).
7. **Fix the `python3 -c` fallback in `install.sh`**. Done since this section was written: `install.sh` has no `python` in it (`grep -c python install.sh` prints 0).
