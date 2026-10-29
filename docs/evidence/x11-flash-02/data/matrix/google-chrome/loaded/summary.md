# xflash analysis

Clock: idle: 1995 ext lines, 574 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 1.204 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.93 ms wide; X11_OBS origin inside that interval (offset from its middle 1.33 ms)
loaded: 1401 ext lines, 260 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 2.896 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 19.61 ms wide; X11_OBS origin inside that interval (offset from its middle 6.96 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| google-chrome | idle | large | 20 | 20 | 0 | 9 | 1 | 0 | 9 | -1.8 / 23.6 | 11.4 | 9/9 (0, 0) | 14.5 / 40.3 |
| google-chrome | idle | small | 20 | 20 | 0 | 14 | 3 | 0 | 14 | 0.8 / 52.4 | 5.8 | 14/14 (0, 0) | 17.2 / 69.0 |
| google-chrome | loaded | large | 20 | 20 | 0 | 15 | 13 | 0 | 15 | 20.5 / 49.5 | 46.3 | 14/15 (0, 1) | 31.2 / 66.1 |
| google-chrome | loaded | small | 20 | 20 | 0 | 4 | 0 | 0 | 4 | -0.6 / 14.6 | 31.3 | 4/4 (0, 0) | 12.8 / 24.0 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
| google-chrome | idle | large | 5 | 0 | 32.2 / 34.4 | 5x reached:at-target | 5 |
| google-chrome | idle | small | 5 | 0 | 31.5 / 34.1 | 5x reached:at-target | 5 |
