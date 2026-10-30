# xflash analysis

Clock: idle: 1936 ext lines, 518 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 0.293 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.80 ms wide; X11_OBS origin inside that interval (offset from its middle 1.33 ms)
loaded: 1663 ext lines, 522 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 5.826 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 10.78 ms wide; X11_OBS origin inside that interval (offset from its middle 2.85 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| qt-assistant | idle | large | 20 | 20 | 0 | 10 | 20 | 0 | 20 | 43.9 / 52.7 | 6.4 | 0/20 (7, 13) | 19.9 / 22.9 |
| qt-assistant | idle | small | 20 | 20 | 0 | 1 | 14 | 0 | 14 | 23.6 / 37.1 | 5.3 | 0/14 (7, 12) | 17.4 / 20.0 |
| qt-assistant | loaded | large | 20 | 20 | 0 | 5 | 6 | 0 | 7 | 21.5 / 67.3 | 48.4 | 4/7 (11, 2) | 23.1 / 38.3 |
| qt-assistant | loaded | small | 20 | 20 | 0 | 2 | 10 | 0 | 11 | 12.5 / 49.8 | 45.3 | 2/11 (11, 3) | 13.3 / 47.3 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
| qt-assistant | idle | large | 5 | 0 | 252.7 / 253.6 | 2x reached:at-target; 3x timed-out:cap | 2 |
| qt-assistant | idle | small | 5 | 0 | 12.4 / 14.1 | 5x reached:at-target | 5 |
