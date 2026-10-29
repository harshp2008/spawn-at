# xflash analysis

Clock: b_smoke: 191 ext lines, 46 X11_OBS clock lines, 3 windows with an origin, per-window origin spread max 0.324 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.96 ms wide; X11_OBS origin inside that interval (offset from its middle 1.31 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| xterm | idle | large | 2 | 2 | 0 | 0 | 0 | 0 | 0 | -0.2 / 0.5 | 36.1 | 0/0 (0, 0) | -8.6 / -8.1 |
| xterm | idle | small | 1 | 1 | 0 | 0 | 0 | 0 | 0 | -1.2 / -1.2 | 35.6 | 0/0 (0, 0) | -11.9 / -11.9 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
