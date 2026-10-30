# xflash analysis

Clock: A-editor-640-gate-warm: 14775 ext lines, 14134 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 0.799 ms, observer dropped 39035 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.26 ms wide; X11_OBS origin inside that interval (offset from its middle 1.59 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-text-editor | idle | large | 10 | 10 | 0 | 0 | 0 | 0 | 0 | -0.3 / 2.8 | 21.3 | 0/0 (0, 0) | -1.1 / 8.6 |
| gnome-text-editor | idle | small | 10 | 10 | 0 | 0 | 0 | 0 | 0 | -0.4 / 2.7 | 17.9 | 0/0 (0, 0) | 2.1 / 9.3 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
