# xflash analysis

Clock: aft-sent1: 1104 ext lines, 753 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 1.006 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.79 ms wide; X11_OBS origin inside that interval (offset from its middle 1.38 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 5 | 5 | 0 | 0 | 0 | 0 | 0 | -7.7 / -0.2 | 21.9 | 0/0 (0, 0) | -0.9 / 2.5 |
| eog | idle | small | 5 | 5 | 0 | 0 | 0 | 0 | 0 | -0.3 / 6.9 | 16.3 | 0/0 (0, 0) | 4.5 / 10.2 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
