# xflash analysis

Clock: pos-xterm: 422 ext lines, 121 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 0.255 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.05 ms wide; X11_OBS origin inside that interval (offset from its middle 1.43 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| xterm | idle | large | 5 | 5 | 0 | 5 | 0 | 0 | 5 | 5.2 / 5.8 | 1.9 | 5/5 (0, 0) | 18.6 / 19.6 |
| xterm | idle | small | 5 | 5 | 0 | 1 | 0 | 0 | 1 | 3.4 / 4.2 | 2.0 | 1/1 (0, 0) | 16.7 / 18.2 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
