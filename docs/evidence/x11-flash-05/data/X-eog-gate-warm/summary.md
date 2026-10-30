# xflash analysis

Clock: X-eog-gate-warm: 462 ext lines, 285 X11_OBS clock lines, 4 windows with an origin, per-window origin spread max 0.454 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.82 ms wide; X11_OBS origin inside that interval (offset from its middle 1.37 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 2 | 2 | 0 | 0 | 0 | 0 | 0 | -3.5 / -0.6 | 25.3 | 0/0 (0, 0) | -3.9 / -2.4 |
| eog | idle | small | 2 | 2 | 0 | 0 | 0 | 0 | 0 | -1.0 / -0.8 | 19.1 | 0/0 (0, 0) | -0.3 / 3.3 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
