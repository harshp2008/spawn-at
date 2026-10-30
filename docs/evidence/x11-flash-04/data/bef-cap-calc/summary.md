# xflash analysis

Clock: bef-cap-calc: 35379 ext lines, 34170 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 0.918 ms, observer dropped 47120 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.32 ms wide; X11_OBS origin inside that interval (offset from its middle 1.60 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-calculator | idle | large | 20 | 20 | 0 | 1 | 0 | 1 | 1 | -0.9 / 3.3 | 8.1 | 1/1 (0, 0) | 15.0 / 15.9 |
| gnome-calculator | idle | small | 20 | 20 | 0 | 0 | 15 | 0 | 15 | 39.1 / 52.9 | 35.5 | 0/15 (0, 15) | -16.5 / -13.7 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
