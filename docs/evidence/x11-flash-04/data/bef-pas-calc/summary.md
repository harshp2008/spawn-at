# xflash analysis

Clock: bef-pas-calc: 44325 ext lines, 43114 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 1.266 ms, observer dropped 47463 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.96 ms wide; X11_OBS origin inside that interval (offset from its middle 1.45 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-calculator | idle | large | 20 | 20 | 0 | 5 | 0 | 0 | 5 | -0.6 / 13.3 | 7.2 | 5/5 (0, 0) | 15.3 / 21.7 |
| gnome-calculator | idle | small | 20 | 20 | 0 | 0 | 15 | 0 | 15 | 36.4 / 52.8 | 36.1 | 0/15 (0, 15) | -17.1 / -13.3 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
