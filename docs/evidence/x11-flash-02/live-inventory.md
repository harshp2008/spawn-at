# Live session inventory (X11-FLASH-02h, step 1)

Read-only. Commands used: `gnome-extensions list/info`, `gsettings` on the allowlist,
`GetCurrentState` and `spawn-at query` for counts, `journalctl` reads, `grep` of extension source.
Nothing on the live session was changed. Live Shell pid 2718897, start time 81373971 before and
after. Tags: MEASURED, INFERRED, HYPOTHESIS.

## Environment (MEASURED)

GNOME Shell 46.0, Mutter 46.2; boot id starts `4b4e9ad0`, up since 2026-10-20 03:12; this Shell
started 13:15 on 2026-10-29. One monitor, 1920x1080, scale 1 (work area 1920x1040), 3 open windows.

## Extensions (MEASURED): 22 enabled, 7 installed but off

User extensions (in the user's data dir): Dash to Dock (v9999), Touchpad Speed Control (5), Swap
Finger Gestures (1), GNOME Fuzzy App Search, Adaptive Dock (v2, the maintainer's own, uuid
`adaptive-dock@harsh.patel`), Caffeine (61), Just Perfection (37), Space Bar (34), GSConnect, Dock
Hover Preview (1.0-beta.1), Spawn-At Interceptor, Win11Clip Bridge, Window Calls (21).
System extensions: Auto Move Windows, Launch new instance, Light Style, Native Window Placement,
System Monitor, User Theme (all gnome-shell-extensions), Desktop Icons NG (ding), Ubuntu Tiling
Assistant (46), Ubuntu AppIndicators. Off: Apps Menu, Drive Menu, Places, Screenshot Window Sizer,
Window List, windowNavigator, Workspace Indicator. No extension named "Dock Focus Tracker" is
installed; the maintainer's own is Adaptive Dock (it listens to `notify::focus-window`).

## Settings (MEASURED, allowlist only)

focus-mode click; auto-raise false; raise-on-click true; focus-new-windows smart; 4 workspaces,
dynamic, only on the primary monitor; edge-tiling false; check-alive-timeout 5000;
experimental-features `['scale-monitor-framebuffer']`; text-scaling-factor 1.25; scaling-factor 0;
animations on; xsettings overrides empty. Extension settings of Adaptive Dock, Dock Hover Preview,
Just Perfection, Space Bar, Touchpad Speed Control are in `data/h7/ext_settings.txt` (GSConnect,
Win11Clip Bridge excluded as personal).

## Warnings in the live journal since boot (MEASURED, `data/h7/warning-counts.txt`)

| Warning | Total since 2026-10-20 | Pattern |
| :-- | --: | :-- |
| stack_position >= 0 assertion | 1233 | every day, 36 to 318 a day: chronic |
| Error in size change accounting | 95 | most days |
| Invalid window geometry for xdg_surface | 1 | 2026-10-29 12:11 |
| scale-x / scale-y "inf" | 2 | 2026-10-29 12:11 |
| Buggy client ... _NET_ACTIVE_WINDOW timestamp 0 | 3 | **2026-10-24 16:08 (no crash)**, and the two crashes |
| Tried to ping ... bad serial | 88 | all on 2026-10-29, from the xflash runs and the two Chrome windows |
| Failed to focus window | 2 | the two crashes |

- The 2026-10-24 16:08 timestamp-0 warning (window 0x1200009) came with no crash, no assertion and
  no spawn-at log line: the message alone does not crash the Shell. A gsconnect clipboard
  "Unhandled promise rejection" is logged in the same second (INFERRED coincidence; no link shown).
- In the 10 minutes before each crash the journal has no JS ERROR and no extension error; the lines
  are the Screencast failures of my own earlier attempt (12:22 to 12:32) and Chrome renderer noise
  (12:56 to 13:06). In the 2 seconds before each assertion the only components logging are the
  Shell's own window-manager warnings (bad serial ping, failed focus). Nothing is attributed to an
  extension (no UUID, class or stack trace beside the chronic warnings).

## Extension sources that react to focus or close (HYPOTHESIS, source reading only)

None of the 22 calls activate or focus on a window it knows is closing. The ones that listen:
- Adaptive Dock (own): `notify::focus-window` handler (extension.js:83), reads `focus_window`
  (:312), `unmanaged` handler that only deletes map entries (:605).
- Ubuntu Tiling Assistant: `unmanaging` handler (tilingWindowManager.js:427) that clears its
  properties; `raised` handler; `nearestWindow.focus()` only on a keybinding
  (keybindingHandler.js:262); active-window hint on `notify::focus-window` (activeWindowHint.js:245).
- Dash to Dock: `notify::focus-window` (locations.js:1010, appIcons.js:967), `focus_window` reads
  (docking.js:930, 969; intellihide.js:270), `unmanaged` in fileManager1API.js:184.
- Dock Hover Preview: `notify::focus-window` (windowCard.js:82), eases window clones
  (windowPeek.js:61, 77).
- Space Bar, DING, GSConnect, Window Calls, Just Perfection: activate/raise calls exist but on user
  action (menu, keybinding, notification).
- Scales: only Dash to Dock sets `scale_x` (dash.js:70, fixed 1). No extension was found animating
  a window actor with a computed scale, so the source of the "inf" scale warning is not found.
So an extension reacting to a focus change at the moment a window unmanages is plausible for the
four listeners above, but no code path that would fail the focus hand-over was seen.
