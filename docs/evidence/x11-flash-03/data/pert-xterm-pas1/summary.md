# xflash analysis

Clock: pert-xterm-pas1: 500 ext lines, 159 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 3.711 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.21 ms wide; X11_OBS origin inside that interval (offset from its middle 1.60 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| xterm | idle | large | 5 | 5 | 0 | 0 | 0 | 0 | 0 | -0.6 / -0.2 | 34.0 | 0/0 (0, 0) | -11.1 / -5.0 |
| xterm | idle | small | 5 | 5 | 0 | 0 | 0 | 0 | 0 | -0.2 / 0.2 | 32.9 | 0/0 (0, 0) | -11.2 / -10.0 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
