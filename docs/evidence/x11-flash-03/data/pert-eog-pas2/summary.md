# xflash analysis

Clock: pert-eog-pas2: 1513 ext lines, 1182 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 0.491 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.47 ms wide; X11_OBS origin inside that interval (offset from its middle 1.58 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 5 | 5 | 0 | 2 | 3 | 0 | 3 | 16.9 / 198.5 | 5.2 | 0/3 (0, 3) | 16.0 / 18.3 |
| eog | idle | small | 5 | 5 | 0 | 4 | 2 | 0 | 5 | 16.5 / 23.8 | 2.8 | 3/5 (0, 2) | 18.4 / 19.2 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
