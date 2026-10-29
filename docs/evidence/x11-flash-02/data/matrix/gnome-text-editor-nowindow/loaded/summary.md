# xflash analysis

Clock: idle: 1732 ext lines, 317 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 0.396 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.17 ms wide; X11_OBS origin inside that interval (offset from its middle 1.53 ms)
loaded: 1446 ext lines, 305 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 10.074 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 11.90 ms wide; X11_OBS origin inside that interval (offset from its middle 3.96 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-text-editor-nowindow | idle | large | 20 | 20 | 0 | 6 | 0 | 0 | 6 | -1.4 / 5.3 | 4.4 | 6/6 (8, 0) | 17.5 / 21.7 |
| gnome-text-editor-nowindow | idle | small | 20 | 20 | 0 | 2 | 0 | 0 | 2 | -1.1 / 1.1 | 4.8 | 2/2 (11, 0) | 15.3 / 17.6 |
| gnome-text-editor-nowindow | loaded | large | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -13.7 / 2.2 | 26.6 | 0/0 (20, 0) | - / - |
| gnome-text-editor-nowindow | loaded | small | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -8.1 / 1.7 | 39.2 | 0/0 (0, 0) | 4.6 / 12.9 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
| gnome-text-editor-nowindow | idle | large | 5 | 0 | 13.8 / 15.0 | 5x reached:at-target | 5 |
| gnome-text-editor-nowindow | idle | small | 5 | 0 | 9.4 / 11.3 | 5x reached:at-target | 5 |
