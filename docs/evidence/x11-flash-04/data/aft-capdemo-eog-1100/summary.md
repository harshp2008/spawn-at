# xflash analysis

Clock: aft-capdemo-eog-1100: 2187 ext lines, 1526 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 1.797 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.29 ms wide; X11_OBS origin inside that interval (offset from its middle 1.55 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 10 | 10 | 0 | 0 | 0 | 0 | 0 | 6.8 / 9.9 | 11.1 | 0/0 (0, 0) | 9.6 / 11.0 |
| eog | idle | small | 10 | 10 | 0 | 0 | 0 | 0 | 0 | -0.3 / 7.7 | 13.0 | 0/0 (0, 0) | 7.6 / 10.6 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
