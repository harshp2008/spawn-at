# xflash analysis

Clock: idle: 1741 ext lines, 320 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 0.224 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.78 ms wide; X11_OBS origin inside that interval (offset from its middle 1.35 ms)
loaded: 1457 ext lines, 304 X11_OBS clock lines, 39 windows with an origin, per-window origin spread max 7.745 ms, observer dropped 0 damage lines; SetLogging brackets: the 90 newest pairs do not overlap (77496.63 ms apart), not used

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 20 | 20 | 0 | 19 | 0 | 0 | 19 | 10.4 / 16.2 | 2.8 | 19/19 (0, 0) | 19.0 / 19.8 |
| eog | idle | small | 20 | 20 | 0 | 20 | 0 | 0 | 20 | 5.3 / 8.8 | 2.8 | 20/20 (0, 0) | 17.0 / 17.4 |
| eog | loaded | large | 20 | 19 | 0 | 10 | 12 | 0 | 15 | 28.1 / 66.9 | 29.3 | 4/15 (0, 11) | 23.8 / 61.3 |
| eog | loaded | small | 20 | 20 | 0 | 0 | 12 | 0 | 12 | 22.3 / 48.4 | 24.4 | 0/12 (0, 12) | 11.4 / 23.1 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
| eog | idle | large | 5 | 0 | 16.4 / 19.2 | 5x reached:at-target | 5 |
| eog | idle | small | 5 | 0 | 11.2 / 14.1 | 5x reached:at-target | 5 |
