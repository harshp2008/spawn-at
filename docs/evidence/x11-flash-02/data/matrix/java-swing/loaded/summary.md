# xflash analysis

Clock: idle: 1566 ext lines, 425 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 0.973 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.67 ms wide; X11_OBS origin inside that interval (offset from its middle 1.36 ms)
loaded: 1565 ext lines, 412 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 10.977 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 12.33 ms wide; X11_OBS origin inside that interval (offset from its middle 4.17 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| java-swing | idle | large | 20 | 20 | 0 | 14 | 20 | 0 | 20 | 31.8 / 46.8 | 7.7 | 0/20 (0, 20) | 15.7 / 20.0 |
| java-swing | idle | small | 20 | 20 | 0 | 0 | 17 | 0 | 17 | 31.7 / 38.1 | 7.9 | 0/17 (14, 6) | 9.7 / 11.4 |
| java-swing | loaded | large | 20 | 20 | 0 | 6 | 3 | 0 | 6 | -7.4 / 23.8 | 61.6 | 6/6 (0, 0) | -9.7 / 37.0 |
| java-swing | loaded | small | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -45.3 / -4.3 | 67.9 | 0/0 (13, 0) | -13.7 / -2.8 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
