# xflash analysis

Clock: tk-chrome: 780 ext lines, 179 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 3.189 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.69 ms wide; X11_OBS origin inside that interval (offset from its middle 1.41 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| google-chrome | idle | large | 10 | 10 | 0 | 2 | 1 | 0 | 2 | -1.4 / 69.3 | 11.3 | 2/2 (0, 0) | 14.4 / 85.8 |
| google-chrome | idle | small | 10 | 10 | 0 | 3 | 0 | 0 | 3 | -1.2 / 16.5 | 5.6 | 3/3 (0, 0) | 14.9 / 33.2 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
