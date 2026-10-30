# xflash analysis

Clock: pert-eog-cap1: 1031 ext lines, 700 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 4.474 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.09 ms wide; X11_OBS origin inside that interval (offset from its middle 1.43 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 5 | 5 | 0 | 3 | 0 | 0 | 3 | 11.2 / 15.0 | 3.4 | 3/3 (0, 0) | 17.3 / 17.9 |
| eog | idle | small | 5 | 5 | 0 | 4 | 2 | 0 | 4 | 15.1 / 26.1 | 3.1 | 2/4 (0, 2) | 18.3 / 18.9 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
