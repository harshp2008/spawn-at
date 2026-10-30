# xflash analysis

Clock: X-qt-legacy-warm: 224 ext lines, 51 X11_OBS clock lines, 4 windows with an origin, per-window origin spread max 0.411 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.81 ms wide; X11_OBS origin inside that interval (offset from its middle 1.35 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| qt-assistant | idle | large | 2 | 2 | 0 | 1 | 2 | 0 | 2 | 60.4 / 69.7 | 6.6 | 0/2 (0, 2) | 18.5 / 21.3 |
| qt-assistant | idle | small | 2 | 2 | 0 | 0 | 2 | 0 | 2 | 22.3 / 22.3 | 9.8 | 0/2 (1, 1) | 9.8 / 9.8 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
