# xflash analysis

Clock: S-kitty: 150 ext lines, 35 X11_OBS clock lines, 2 windows with an origin, per-window origin spread max 0.443 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.30 ms wide; X11_OBS origin inside that interval (offset from its middle 1.62 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| kitty | idle | large | 1 | 1 | 0 | 1 | 0 | 0 | 1 | 2.8 / 2.8 | 4.8 | 1/1 (0, 0) | 19.5 / 19.5 |
| kitty | idle | small | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0.3 / 0.3 | 4.8 | 0/0 (0, 0) | 15.3 / 15.3 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
