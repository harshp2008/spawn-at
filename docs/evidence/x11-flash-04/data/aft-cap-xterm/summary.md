# xflash analysis

Clock: aft-cap-xterm: 978 ext lines, 317 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 0.452 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.51 ms wide; X11_OBS origin inside that interval (offset from its middle 1.23 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| xterm | idle | large | 10 | 10 | 0 | 0 | 0 | 0 | 0 | -0.4 / -0.1 | 32.7 | 0/0 (0, 0) | -11.6 / -2.4 |
| xterm | idle | small | 10 | 10 | 0 | 0 | 0 | 0 | 0 | -0.2 / -0.0 | 32.2 | 0/0 (0, 0) | -13.7 / -12.9 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
