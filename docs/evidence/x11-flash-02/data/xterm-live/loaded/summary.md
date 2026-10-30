# xflash analysis

Clock: b_xterm_idle: 1776 ext lines, 595 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 0.555 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.28 ms wide; X11_OBS origin inside that interval (offset from its middle 1.49 ms)
b_xterm_loaded: 1651 ext lines, 468 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 2.404 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 18.39 ms wide; X11_OBS origin inside that interval (offset from its middle 7.51 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| xterm | idle | large | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -10.5 / -4.0 | 47.5 | 0/0 (0, 0) | -18.6 / -10.5 |
| xterm | idle | small | 20 | 20 | 0 | 0 | 1 | 0 | 1 | -11.0 / 36.6 | 47.4 | 0/1 (0, 1) | -21.2 / -12.5 |
| xterm | loaded | large | 20 | 20 | 0 | 4 | 6 | 0 | 7 | 2.4 / 88.2 | 65.5 | 1/7 (0, 6) | -0.4 / 41.7 |
| xterm | loaded | small | 20 | 20 | 0 | 6 | 6 | 0 | 7 | -7.5 / 96.5 | 60.6 | 5/7 (0, 2) | -6.9 / 102.2 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
