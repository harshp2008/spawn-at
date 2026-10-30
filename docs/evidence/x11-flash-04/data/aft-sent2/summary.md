# xflash analysis

Clock: aft-sent2: 1127 ext lines, 775 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 1.041 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.74 ms wide; X11_OBS origin inside that interval (offset from its middle 1.24 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 5 | 5 | 0 | 0 | 0 | 0 | 0 | -0.2 / -0.2 | 17.5 | 0/0 (0, 0) | 3.0 / 3.8 |
| eog | idle | small | 5 | 5 | 0 | 0 | 0 | 0 | 0 | -0.4 / -0.0 | 16.5 | 0/0 (0, 0) | 4.3 / 8.4 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
