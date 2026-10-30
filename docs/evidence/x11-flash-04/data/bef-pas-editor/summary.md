# xflash analysis

Clock: bef-pas-editor: 34421 ext lines, 33280 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 0.735 ms, observer dropped 32501 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.95 ms wide; X11_OBS origin inside that interval (offset from its middle 1.45 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-text-editor | idle | large | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -0.7 / -0.3 | 4.9 | 0/0 (11, 0) | 15.1 / 15.8 |
| gnome-text-editor | idle | small | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -0.7 / -0.3 | 4.9 | 0/0 (10, 0) | 15.2 / 16.1 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
