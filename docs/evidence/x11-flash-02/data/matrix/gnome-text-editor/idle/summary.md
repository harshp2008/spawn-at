# xflash analysis

Clock: idle: 1723 ext lines, 308 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 0.202 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.53 ms wide; X11_OBS origin inside that interval (offset from its middle 1.24 ms)
loaded: 1453 ext lines, 308 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 1.386 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 17.29 ms wide; X11_OBS origin inside that interval (offset from its middle 6.42 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-text-editor | idle | large | 20 | 20 | 0 | 3 | 2 | 0 | 4 | -1.6 / 48.3 | 5.3 | 2/4 (10, 1) | 15.1 / 20.4 |
| gnome-text-editor | idle | small | 20 | 20 | 0 | 2 | 1 | 0 | 2 | -1.0 / 50.1 | 5.3 | 1/2 (9, 1) | 15.4 / 18.1 |
| gnome-text-editor | loaded | large | 20 | 20 | 0 | 4 | 3 | 0 | 4 | -3.0 / 26.5 | 35.7 | 4/4 (0, 0) | 8.1 / 34.1 |
| gnome-text-editor | loaded | small | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -10.4 / -2.8 | 21.7 | 0/0 (20, 0) | - / - |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
| gnome-text-editor | idle | large | 5 | 0 | 12.0 / 14.7 | 5x reached:at-target | 5 |
| gnome-text-editor | idle | small | 5 | 0 | 11.1 / 11.7 | 5x reached:at-target | 5 |
