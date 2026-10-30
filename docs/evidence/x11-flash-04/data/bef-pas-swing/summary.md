# xflash analysis

Clock: bef-pas-swing: 836 ext lines, 232 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 0.877 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.68 ms wide; X11_OBS origin inside that interval (offset from its middle 1.70 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| java-swing | idle | large | 10 | 10 | 0 | 7 | 9 | 0 | 10 | 37.3 / 48.9 | 3.3 | 1/10 (0, 9) | 18.0 / 18.5 |
| java-swing | idle | small | 10 | 10 | 0 | 0 | 9 | 0 | 9 | 33.7 / 42.2 | 4.0 | 0/9 (10, 0) | - / - |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
