# xflash analysis

Clock: aft-pas-calc: 52513 ext lines, 51235 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 2.055 ms, observer dropped 49319 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.69 ms wide; X11_OBS origin inside that interval (offset from its middle 1.33 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-calculator | idle | large | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -76.7 / 7.1 | 82.0 | 0/0 (0, 0) | -60.9 / 9.3 |
| gnome-calculator | idle | small | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -0.5 / 7.2 | 86.5 | 0/0 (0, 0) | -66.7 / -53.3 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
