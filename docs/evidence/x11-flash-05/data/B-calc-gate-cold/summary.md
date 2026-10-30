# xflash analysis

Clock: B-calc-gate-cold: 28922 ext lines, 27701 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 2.485 ms, observer dropped 26787 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.95 ms wide; X11_OBS origin inside that interval (offset from its middle 1.32 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-calculator | idle | large | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -73.7 / -64.2 | 79.0 | 0/0 (0, 0) | -57.8 / -47.5 |
| gnome-calculator | idle | small | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -79.3 / -66.7 | 83.3 | 0/0 (0, 0) | -63.2 / -50.0 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
