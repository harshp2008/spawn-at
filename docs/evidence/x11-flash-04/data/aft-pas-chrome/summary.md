# xflash analysis

Clock: aft-pas-chrome: 971 ext lines, 330 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 3.865 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.84 ms wide; X11_OBS origin inside that interval (offset from its middle 1.27 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| google-chrome | idle | large | 10 | 10 | 0 | 1 | 1 | 0 | 1 | -0.3 / 53.3 | 14.7 | 1/1 (0, 0) | 8.7 / 69.4 |
| google-chrome | idle | small | 10 | 10 | 0 | 2 | 1 | 0 | 2 | -0.4 / 20.9 | 13.9 | 2/2 (0, 0) | 6.9 / 37.5 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
