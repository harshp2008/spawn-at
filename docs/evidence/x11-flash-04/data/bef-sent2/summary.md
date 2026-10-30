# xflash analysis

Clock: bef-sent2: 1057 ext lines, 726 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 0.904 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.78 ms wide; X11_OBS origin inside that interval (offset from its middle 1.26 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 5 | 5 | 0 | 5 | 2 | 0 | 5 | 14.2 / 44.9 | 2.5 | 3/5 (0, 2) | 18.0 / 19.1 |
| eog | idle | small | 5 | 5 | 0 | 5 | 1 | 0 | 5 | 11.4 / 24.4 | 2.4 | 4/5 (0, 1) | 17.8 / 18.5 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
