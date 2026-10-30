# xflash analysis

Clock: aft-cap-eog-1700: 1741 ext lines, 1100 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 1.981 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.51 ms wide; X11_OBS origin inside that interval (offset from its middle 1.75 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 10 | 10 | 0 | 0 | 0 | 0 | 0 | -0.3 / 15.2 | 24.9 | 0/0 (0, 0) | -0.1 / 3.5 |
| eog | idle | small | 10 | 10 | 0 | 0 | 0 | 0 | 0 | -0.2 / 13.5 | 23.2 | 0/0 (0, 0) | 1.2 / 3.9 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
