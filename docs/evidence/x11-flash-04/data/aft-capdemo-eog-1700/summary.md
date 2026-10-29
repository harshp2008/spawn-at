# xflash analysis

Clock: aft-capdemo-eog-1700: 1013 ext lines, 652 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 0.578 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.09 ms wide; X11_OBS origin inside that interval (offset from its middle 1.52 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 5 | 5 | 0 | 0 | 3 | 0 | 3 | 18.8 / 30.2 | 8.1 | 0/3 (0, 3) | 15.7 / 16.2 |
| eog | idle | small | 5 | 5 | 0 | 0 | 0 | 0 | 0 | 13.0 / 14.1 | 8.7 | 0/0 (0, 0) | 15.3 / 16.0 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
