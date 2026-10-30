# xflash analysis

Clock: S-gnome-system-monitor: 142 ext lines, 24 X11_OBS clock lines, 2 windows with an origin, per-window origin spread max 0.343 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.78 ms wide; X11_OBS origin inside that interval (offset from its middle 1.79 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-system-monitor | idle | large | 1 | 1 | 0 | 1 | 1 | 1 | 1 | 486.8 / 486.8 | 9.9 | 0/1 (0, 1) | 21.5 / 21.5 |
| gnome-system-monitor | idle | small | 1 | 1 | 0 | 0 | 0 | 0 | 0 | -184.5 / -184.5 | 3.8 | 0/0 (1, 0) | - / - |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
