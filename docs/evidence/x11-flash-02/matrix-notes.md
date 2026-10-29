# X11-FLASH-02i-b per-app matrix (private headless Shell, DING not loaded)

Every batch below ran in the one private headless GNOME Shell of this run (own D-Bus, Wayland
socket, Xwayland and keyfile gsettings; enabled extensions `['spawn-at@harsh.local']`, DING
installed but disabled). Live Shell pid 2718897 / start time 81373971 did not change before or
after any batch. Numbers are the output of `xflash analyze`; V, R, D, A and L are the definitions
in `PLAN.md`.

## gnome-text-editor (GTK4)

Private headless Shell, DING not loaded. Forced X11 (`GDK_BACKEND=x11`), 20 runs per size idle plus
10 native Wayland controls (5 per size), then 20 runs per size loaded.

### X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: |
| gnome-text-editor | idle | large | 20 | 20 | 0 | 3 | 2 | 0 | 4 | -1.6 / 48.3 | 5.3 |
| gnome-text-editor | idle | small | 20 | 20 | 0 | 2 | 1 | 0 | 2 | -1.0 / 50.1 | 5.3 |
| gnome-text-editor | loaded | large | 20 | 20 | 0 | 4 | 3 | 0 | 4 | -3.0 / 26.5 | 35.7 |
| gnome-text-editor | loaded | small | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -10.4 / -2.8 | 21.7 |

### Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
| gnome-text-editor | idle | large | 5 | 0 | 12.0 / 14.7 | 5x reached:at-target | 5 |
| gnome-text-editor | idle | small | 5 | 0 | 11.1 / 11.7 | 5x reached:at-target | 5 |

Note: the first loaded attempt stopped at run 0 (`no_window` under load; the window appeared after
`spawn-at spawn` had returned). The window was closed gracefully, the partial batch directory was
removed, and the batch was restarted from scratch with the same seed; the table above is the clean
40-run batch. The raw `xobs.jsonl` of each batch was reduced to its first 200 lines
(`xobs.head.jsonl`, see `README.txt` in each directory) after analysis to stay under the evidence
size budget; `runs.csv` keeps the per-run values.

## eog (GTK3)

Private headless Shell, DING not loaded. Forced X11 (`GDK_BACKEND=x11`), 20 runs per size idle plus
10 native Wayland controls (5 per size), then 20 runs per size loaded.

### X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: |
| eog | idle | large | 20 | 20 | 0 | 19 | 0 | 0 | 19 | 10.4 / 16.2 | 2.8 |
| eog | idle | small | 20 | 20 | 0 | 20 | 0 | 0 | 20 | 5.3 / 8.8 | 2.8 |
| eog | loaded | large | 20 | 19 | 0 | 10 | 12 | 0 | 15 | 28.1 / 66.9 | 29.3 |
| eog | loaded | small | 20 | 20 | 0 | 0 | 12 | 0 | 12 | 22.3 / 48.4 | 24.4 |

### Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
| eog | idle | large | 5 | 0 | 16.4 / 19.2 | 5x reached:at-target | 5 |
| eog | idle | small | 5 | 0 | 11.2 / 14.1 | 5x reached:at-target | 5 |

Note: the loaded batch's run 0 (large) was `no_window` under load (the window appeared after
`spawn-at spawn` returned, with its default size); the late window was closed and the batch resumed
with the same seed, so loaded large has valid n=19 and the loaded clock cross-check did not use
the appended SetLogging pairs. The batch itself did not crash the Shell. Full `xobs.jsonl` reduced
to `xobs.head.jsonl` after analysis (see each directory's `README.txt`).

## gnome-calculator (GTK4)

Private headless Shell, DING not loaded. Forced X11 (`GDK_BACKEND=x11`), 20 runs per size idle plus
10 native Wayland controls (5 per size), then 20 runs per size loaded.

### X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: |
| gnome-calculator | idle | large | 20 | 20 | 0 | 4 | 0 | 0 | 4 | -1.1 / 9.1 | 7.0 |
| gnome-calculator | idle | small | 20 | 20 | 0 | 0 | 14 | 0 | 14 | 34.3 / 38.7 | 34.9 |
| gnome-calculator | loaded | large | 20 | 20 | 0 | 1 | 0 | 0 | 1 | -7.6 / 4.7 | 37.9 |
| gnome-calculator | loaded | small | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -6.4 / 0.4 | 64.8 |

### Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
| gnome-calculator | idle | large | 5 | 0 | 13.1 / 15.8 | 5x reached:at-target | 5 |
| gnome-calculator | idle | small | 5 | 0 | 42.3 / 44.0 | 5x adjusted:quiet | 0 |

Note (applies to this and the following loaded batches): the private sandbox's `SPAWN_AT_CONFIG`
file (`$SB/none.toml`, inside the sandbox only) sets `spawn.window_timeout_ms = 15000`. The first
two loaded attempts with the default (2000 ms, total wait at most twice that) returned `no_window`:
under the 22-thread load the calculator window appeared at ~5 s, after `spawn-at spawn` had
returned; the late windows were closed and the partial batch removed before the clean 40-run batch
above. This setting only extends how long `spawn-at` waits for a window that appears late; it does
not change the extension's cloak/reveal timing used by the analyser. The xterm calibration and the
gnome-text-editor/eog batches used the default. Full `xobs.jsonl` reduced to `xobs.head.jsonl`.

## gnome-terminal (GTK3, server)

Private headless Shell, DING not loaded. Forced X11 (`GDK_BACKEND=x11`), 20 runs per size idle plus
10 native Wayland controls (5 per size), then 20 runs per size loaded.

The batch tool started and closed a window on every run, but every window in the extension log was
`proto=Wayland` (class `gnome-terminal-server`): the D-Bus-activated server, not the launched client,
owns the window's protocol. The analyzer therefore has no X11 clock lines and no valid X11 rows.

### X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: |
| gnome-terminal | idle | large | 20 | 0 | 0 | 0 | 0 | 0 | 0 | - / - | - |
| gnome-terminal | idle | small | 20 | 0 | 0 | 0 | 0 | 0 | 0 | - / - | - |
| gnome-terminal | loaded | large | 20 | 0 | 0 | 0 | 0 | 0 | 0 | - / - | - |
| gnome-terminal | loaded | small | 20 | 0 | 0 | 0 | 0 | 0 | 0 | - / - | - |

### Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
| gnome-terminal | idle | large | 5 | 0 | - / - | 5x adjusted:quiet | 0 |
| gnome-terminal | idle | small | 5 | 0 | - / - | 5x adjusted:quiet | 0 |

Note: this is the server caveat listed in `PLAN.md`. No X11 number can be reported for this app with
the unmodified batch tool; it did not crash the Shell. Full `xobs.jsonl` reduced to
`xobs.head.jsonl`.
