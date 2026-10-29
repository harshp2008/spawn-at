# xflash analysis

Clock: bef-cap-eog-600-1100: 5002 ext lines, 3861 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 0.984 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.94 ms wide; X11_OBS origin inside that interval (offset from its middle 1.31 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 20 | 20 | 0 | 17 | 5 | 0 | 17 | 13.1 / 23.8 | 2.4 | 12/17 (0, 5) | 18.3 / 19.8 |
| eog | idle | small | 20 | 20 | 0 | 1 | 1 | 0 | 2 | 6.6 / 35.5 | 2.2 | 1/2 (0, 1) | 16.3 / 16.8 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
