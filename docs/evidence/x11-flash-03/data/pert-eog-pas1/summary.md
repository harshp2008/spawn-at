# xflash analysis

Clock: pert-eog-pas1: 1624 ext lines, 1293 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 0.531 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.82 ms wide; X11_OBS origin inside that interval (offset from its middle 1.40 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 5 | 5 | 0 | 4 | 0 | 0 | 4 | 9.8 / 15.9 | 2.9 | 4/4 (0, 0) | 17.9 / 18.5 |
| eog | idle | small | 5 | 5 | 0 | 4 | 0 | 0 | 4 | 15.2 / 16.4 | 2.5 | 4/4 (0, 0) | 18.6 / 19.0 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
