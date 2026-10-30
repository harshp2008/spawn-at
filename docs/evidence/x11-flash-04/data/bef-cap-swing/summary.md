# xflash analysis

Clock: bef-cap-swing: 840 ext lines, 236 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 0.580 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.72 ms wide; X11_OBS origin inside that interval (offset from its middle 1.28 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| java-swing | idle | large | 10 | 10 | 0 | 8 | 10 | 0 | 10 | 40.1 / 58.5 | 3.2 | 0/10 (0, 10) | 17.7 / 19.0 |
| java-swing | idle | small | 10 | 10 | 0 | 0 | 8 | 0 | 8 | 33.1 / 44.8 | 2.8 | 0/8 (10, 0) | - / - |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
