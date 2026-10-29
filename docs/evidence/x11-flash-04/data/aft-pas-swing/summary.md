# xflash analysis

Clock: aft-pas-swing: 891 ext lines, 250 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 0.431 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.26 ms wide; X11_OBS origin inside that interval (offset from its middle 1.10 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| java-swing | idle | large | 10 | 10 | 0 | 0 | 10 | 0 | 10 | 29.9 / 37.8 | 12.5 | 0/10 (0, 10) | 9.2 / 13.3 |
| java-swing | idle | small | 10 | 10 | 0 | 0 | 10 | 0 | 10 | 33.8 / 43.2 | 2.8 | 0/10 (7, 3) | 14.2 / 14.3 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
