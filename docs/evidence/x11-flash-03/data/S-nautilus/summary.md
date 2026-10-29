# xflash analysis

Clock: S-nautilus: 730 ext lines, 615 X11_OBS clock lines, 2 windows with an origin, per-window origin spread max 0.642 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.00 ms wide; X11_OBS origin inside that interval (offset from its middle 1.45 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| nautilus | idle | large | 1 | 1 | 0 | 1 | 0 | 0 | 1 | 5.7 / 5.7 | 3.8 | 1/1 (0, 0) | 22.0 / 22.0 |
| nautilus | idle | small | 1 | 1 | 0 | 0 | 0 | 0 | 0 | -0.2 / -0.2 | 5.1 | 0/0 (0, 0) | 15.6 / 15.6 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
