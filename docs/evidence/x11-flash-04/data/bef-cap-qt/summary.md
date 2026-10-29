# xflash analysis

Clock: bef-cap-qt: 858 ext lines, 257 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 0.469 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.86 ms wide; X11_OBS origin inside that interval (offset from its middle 1.34 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| qt-assistant | idle | large | 10 | 10 | 0 | 5 | 10 | 0 | 10 | 46.9 / 53.1 | 3.4 | 0/10 (4, 6) | 19.1 / 22.5 |
| qt-assistant | idle | small | 10 | 10 | 0 | 0 | 8 | 0 | 8 | 30.1 / 52.4 | 3.4 | 0/8 (4, 6) | 16.3 / 16.6 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
