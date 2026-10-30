# xflash analysis

Clock: pert-xterm-pas2: 514 ext lines, 173 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 0.582 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.34 ms wide; X11_OBS origin inside that interval (offset from its middle 1.60 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| xterm | idle | large | 5 | 5 | 0 | 0 | 0 | 0 | 0 | -0.7 / -0.1 | 33.7 | 0/0 (0, 0) | -11.7 / -9.4 |
| xterm | idle | small | 5 | 5 | 0 | 0 | 0 | 0 | 0 | -0.3 / -0.2 | 33.1 | 0/0 (0, 0) | -11.4 / -9.0 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
