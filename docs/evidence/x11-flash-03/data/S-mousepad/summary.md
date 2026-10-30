# xflash analysis

Clock: S-mousepad: 144 ext lines, 29 X11_OBS clock lines, 2 windows with an origin, per-window origin spread max 0.718 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.77 ms wide; X11_OBS origin inside that interval (offset from its middle 1.29 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| mousepad | idle | large | 1 | 1 | 0 | 1 | 1 | 0 | 1 | 21.5 / 21.5 | 2.2 | 1/1 (0, 0) | 24.9 / 24.9 |
| mousepad | idle | small | 1 | 1 | 0 | 1 | 0 | 0 | 1 | 9.0 / 9.0 | 2.5 | 1/1 (0, 0) | 18.1 / 18.1 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
