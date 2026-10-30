# xflash analysis

Clock: A-fix-editor: 6723 ext lines, 6392 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 0.921 ms, observer dropped 14870 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.59 ms wide; X11_OBS origin inside that interval (offset from its middle 1.17 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-text-editor | idle | large | 5 | 5 | 0 | 0 | 0 | 0 | 0 | -0.9 / -0.4 | 8.3 | 0/0 (2, 0) | 15.6 / 15.8 |
| gnome-text-editor | idle | small | 5 | 5 | 0 | 0 | 0 | 0 | 0 | -0.6 / -0.4 | 4.6 | 0/0 (2, 0) | 15.8 / 16.0 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
