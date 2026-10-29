# xflash analysis

Clock: pert-xterm-cap1: 514 ext lines, 173 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 0.486 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.15 ms wide; X11_OBS origin inside that interval (offset from its middle 1.52 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| xterm | idle | large | 5 | 5 | 0 | 0 | 0 | 0 | 0 | -0.3 / -0.2 | 33.1 | 0/0 (0, 0) | -10.7 / -8.5 |
| xterm | idle | small | 5 | 5 | 0 | 0 | 0 | 0 | 0 | -0.5 / -0.4 | 33.2 | 0/0 (0, 0) | -11.5 / -10.6 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
