# xflash analysis

Clock: aft-pas-eog-1700: 3780 ext lines, 3139 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 0.595 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.93 ms wide; X11_OBS origin inside that interval (offset from its middle 1.39 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 10 | 10 | 0 | 0 | 0 | 0 | 0 | -0.4 / 11.1 | 25.1 | 0/0 (0, 0) | 0.6 / 4.6 |
| eog | idle | small | 10 | 10 | 0 | 0 | 0 | 0 | 0 | -0.3 / -0.3 | 23.8 | 0/0 (0, 0) | 0.9 / 3.6 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
