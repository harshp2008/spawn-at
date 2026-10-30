# xflash analysis

Clock: pert-eog-cap2: 843 ext lines, 511 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 4.169 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.28 ms wide; X11_OBS origin inside that interval (offset from its middle 1.49 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 5 | 5 | 0 | 5 | 3 | 0 | 5 | 21.6 / 27.9 | 2.6 | 2/5 (0, 3) | 19.0 / 20.1 |
| eog | idle | small | 5 | 5 | 0 | 4 | 3 | 0 | 5 | 19.4 / 27.3 | 3.1 | 2/5 (0, 3) | 18.1 / 18.9 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
