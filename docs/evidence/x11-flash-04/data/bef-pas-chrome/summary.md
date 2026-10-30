# xflash analysis

Clock: bef-pas-chrome: 868 ext lines, 267 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 2.697 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.76 ms wide; X11_OBS origin inside that interval (offset from its middle 1.34 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| google-chrome | idle | large | 10 | 10 | 0 | 3 | 0 | 0 | 3 | -0.9 / 4.5 | 11.4 | 3/3 (0, 0) | 14.9 / 21.1 |
| google-chrome | idle | small | 10 | 10 | 0 | 1 | 1 | 0 | 1 | -1.2 / 67.3 | 5.5 | 1/1 (0, 0) | 14.5 / 83.9 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
