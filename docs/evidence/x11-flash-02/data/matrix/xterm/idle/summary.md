# xflash analysis

Clock: idle: 1779 ext lines, 598 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 0.418 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.41 ms wide; X11_OBS origin inside that interval (offset from its middle 1.17 ms)
loaded: 1581 ext lines, 400 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 4.745 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 23.31 ms wide; X11_OBS origin inside that interval (offset from its middle 8.49 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| xterm | idle | large | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -0.1 / 1.6 | 34.7 | 0/0 (0, 0) | -10.4 / -8.1 |
| xterm | idle | small | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -0.5 / -0.2 | 34.9 | 0/0 (0, 0) | -12.6 / -9.3 |
| xterm | loaded | large | 20 | 20 | 0 | 4 | 5 | 0 | 5 | -2.9 / 308.9 | 79.9 | 1/5 (0, 4) | -15.1 / 63.7 |
| xterm | loaded | small | 20 | 20 | 0 | 2 | 1 | 0 | 2 | -5.8 / 168.2 | 73.7 | 1/2 (0, 1) | -12.8 / 22.1 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
