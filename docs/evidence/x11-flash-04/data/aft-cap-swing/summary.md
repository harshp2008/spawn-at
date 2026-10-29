# xflash analysis

Clock: aft-cap-swing: 887 ext lines, 246 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 0.504 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.83 ms wide; X11_OBS origin inside that interval (offset from its middle 1.36 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| java-swing | idle | large | 10 | 10 | 0 | 0 | 10 | 0 | 10 | 40.2 / 46.2 | 12.5 | 0/10 (0, 10) | 8.6 / 13.1 |
| java-swing | idle | small | 10 | 10 | 0 | 0 | 9 | 0 | 9 | 35.3 / 41.9 | 3.2 | 0/9 (9, 1) | 13.5 / 13.5 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
