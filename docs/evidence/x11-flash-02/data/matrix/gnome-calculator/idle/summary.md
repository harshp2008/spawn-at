# xflash analysis

Clock: idle: 1794 ext lines, 319 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 0.350 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.11 ms wide; X11_OBS origin inside that interval (offset from its middle 1.40 ms)
loaded: 1495 ext lines, 312 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 5.256 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 12.64 ms wide; X11_OBS origin inside that interval (offset from its middle 4.45 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-calculator | idle | large | 20 | 20 | 0 | 4 | 0 | 0 | 4 | -1.1 / 9.1 | 7.0 | 4/4 (0, 0) | 15.2 / 18.8 |
| gnome-calculator | idle | small | 20 | 20 | 0 | 0 | 14 | 0 | 14 | 34.3 / 38.7 | 34.9 | 0/14 (0, 14) | -15.5 / -13.0 |
| gnome-calculator | loaded | large | 20 | 20 | 0 | 1 | 0 | 0 | 1 | -7.6 / 4.7 | 37.9 | 1/1 (0, 0) | 3.0 / 19.1 |
| gnome-calculator | loaded | small | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -6.4 / 0.4 | 64.8 | 0/0 (0, 0) | -31.9 / -17.7 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
| gnome-calculator | idle | large | 5 | 0 | 13.1 / 15.8 | 5x reached:at-target | 5 |
| gnome-calculator | idle | small | 5 | 0 | 42.3 / 44.0 | 5x adjusted:quiet | 0 |
