# Checkpoint D: native Wayland per toolkit

Task D, measured 2026-10-30 ~21:57–22:11. Live pair 5787 / 9315, unchanged. Capture mode.
`native` = no `GDK_BACKEND=x11` / `QT_QPA_PLATFORM=xcb` / `--ozone-platform=x11`; the CLI waits
with the ready-wait machine (no texture gate for native Wayland).

| Cell | App | Runs | Sizes | bad texture | bad pixels | final = request | anchored | cloak→reveal med (min/max) |
| :-- | :-- | --: | :-- | --: | --: | --: | --: | :-- |
| `N-neg-editor` (reused) | gnome-text-editor native, cold | 20 | 640x420, 1100x760 | 0 | 0 | 20 | 20 | 3.4 (2.6/52.5) |
| `D-editor-native-warm` | gnome-text-editor native, keeper | 10 | 640x420, 1100x760 | 0 | 0 | 10 | 10 | 4.7 (1.9/8.5) |
| `D-eog-image-native` | eog native, **real 3000x2000 gradient** | 20 | 600x420, 1100x760 | 0 | 0 | 20 | 20 | 13.1 (9.3/16.7) |
| `D-qt-native` | Qt assistant, `QT_QPA_PLATFORM=wayland` | 20 | 700x500, 1000x700 | 0 | 0 | 20 | 20 | 5.8 (3.8/14.9) |
| `D-chrome-native` | Chrome, `--ozone-platform=wayland`, fresh profile | 20 | 700x480, 1200x800 | 0 | 0 | 20 | 20 | 32.1 (26.9/69.0) |
| `B-calc-native-cold` (reused) | gnome-calculator native, cold | 40 | 900x700 | 0 | 0 | 40 | 40 | 11.4 (8.3/20.2) |

- **0 bad frames in 130 native runs.** Every run ends at the requested size and at the bottom-right
  anchor (x+w=1896, y+h=1056, work area minus the 24 px margin), checked from `runs.jsonl`.
- The real eog image works: the window shows the image; 0 of 20 bad. The 03b eog "no image" cells
  are not comparable at the picture level, only at the frame level.
- Warm applies only to the editor (single-instance). For the calculator a second launch creates no
  window (Task B); eog runs with `--new-instance` and Qt/Chrome start a new process each time, so a
  "warm" keeper for them is only a shared-system-cache state, not a single-instance one. Chrome
  with one profile was only measured in the X11 mode (Task A).
- `cloak→reveal` is from the extension log, capture mode (the 03b caveat: capture inflates
  durations for fast apps; 04's passive native medians were 7.9/5.9/37.5/8.8/12.4/7.6 ms, so these
  are in the same range). No passive native cell was run in 05.

## Not run

Typing, clipboard (copy/paste), and drag-and-drop checks: **not run**. The private Shell is
headless with no input seat; there is no way to type or drag, and no clipboard owner. Also not run:
touch/pen input, fractional scaling, multi-monitor, the live session.

## Known Wayland caveats per app (READ)

| App | Caveat | Source |
| :-- | :-- | :-- |
| GTK4 editor, calculator, eog | GDK picks Wayland when `GDK_BACKEND=wayland` (or when no X11 display is set); GTK documents that backend environment variables are debugging aids, not API-stable configuration. Client-side decorations/header bars replace the X11 frame, so `buffer_rect` and frame rects differ from X11. | https://docs.gtk.org/gtk4/running.html (environment variables; backend pages linked there: https://docs.gtk.org/gtk4/wayland.html) |
| gnome-calculator | Single-instance GApplication: a second launch activates the existing window; there is no `--new-window` that opens a second window in this build (MEASURED). | measured, no doc page for this behaviour |
| eog | GTK3 image viewer; single-instance via D-Bus is optional and `--new-instance` bypasses it. The README lists GTK, dconf, gdk-pixbuf and D-Bus as requirements. | https://gitlab.gnome.org/GNOME/eog/-/raw/master/README.md |
| Qt 5.15 (assistant) | Qt clients choose the platform at startup; Qt 5.15 documents that any Qt program can run as a Wayland client, with `QT_QPA_PLATFORM=wayland`. Qt Wayland has no global window positioning; the compositor decides placement. | https://doc.qt.io/qt-5/wayland-and-qt.html |
| Chrome (Chromium) | Ozone is the platform layer; `--ozone-platform` selects the platform at runtime when multiple are built in. Wayland support is the `SUPPORTS_OZONE_WAYLAND` build flag. Chrome's window is client-side decorated and the browser paints its own content. | https://chromium.googlesource.com/chromium/src/+/main/docs/ozone_overview.md |

Consequences relevant to spawn-at: on native Wayland the compositor decides placement and spawn-at
uses the ready-wait machine, so the X11 frame/texture race does not exist; the hidden cost is the
ready-wait time (above) and, for Chrome/Qt, a slower first paint. The user-visible risk is
toolkit-dependent CSD/size negotiation, not a flash; 0 bad frames in 130 runs supports that for
these apps on Mutter 46.
