# xflash analysis

Clock: rep-eog-large-3: 1002 ext lines, 671 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 0.979 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.05 ms wide; X11_OBS origin inside that interval (offset from its middle 1.44 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 5 | 5 | 0 | 4 | 2 | 0 | 4 | 14.5 / 33.7 | 2.3 | 2/4 (0, 2) | 18.2 / 19.1 |
| eog | idle | small | 5 | 5 | 0 | 3 | 2 | 0 | 3 | 16.2 / 29.8 | 3.7 | 1/3 (0, 2) | 17.9 / 18.5 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
