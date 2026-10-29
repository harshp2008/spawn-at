# xflash analysis

Clock: aft-off-eog-1100: 2176 ext lines, 1555 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 0.910 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.84 ms wide; X11_OBS origin inside that interval (offset from its middle 1.31 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 10 | 10 | 0 | 8 | 2 | 0 | 8 | 12.0 / 23.7 | 2.5 | 6/8 (0, 2) | 18.2 / 19.3 |
| eog | idle | small | 10 | 10 | 0 | 8 | 4 | 0 | 8 | 12.5 / 24.0 | 2.3 | 4/8 (0, 4) | 18.4 / 19.0 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
