# xflash analysis

Clock: aft-pas-qt: 952 ext lines, 310 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 0.454 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.84 ms wide; X11_OBS origin inside that interval (offset from its middle 1.38 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| qt-assistant | idle | large | 10 | 10 | 0 | 0 | 9 | 0 | 9 | 31.3 / 37.5 | 11.6 | 0/9 (4, 5) | 13.4 / 16.5 |
| qt-assistant | idle | small | 10 | 10 | 0 | 0 | 6 | 0 | 6 | 22.8 / 32.1 | 7.1 | 0/6 (4, 5) | 11.7 / 13.2 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
