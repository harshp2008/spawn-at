# xflash analysis

Clock: C-sync-qt: 147 ext lines, 28 X11_OBS clock lines, 2 windows with an origin, per-window origin spread max 0.385 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.55 ms wide; X11_OBS origin inside that interval (offset from its middle 1.23 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| qt-assistant | idle | large | 1 | 1 | 0 | 0 | 1 | 0 | 1 | 28.6 / 28.6 | 14.1 | 0/1 (0, 1) | 17.5 / 17.5 |
| qt-assistant | idle | small | 1 | 1 | 0 | 0 | 1 | 0 | 1 | - / - | 4.2 | 0/1 (1, 0) | - / - |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
