# xflash analysis

Clock: aft-cap-eog-600-1100: 4997 ext lines, 3776 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 1.043 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.69 ms wide; X11_OBS origin inside that interval (offset from its middle 1.31 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -0.5 / -0.0 | 16.8 | 0/0 (0, 0) | 3.6 / 7.2 |
| eog | idle | small | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -0.3 / -0.1 | 10.1 | 0/0 (0, 0) | 8.6 / 10.7 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
