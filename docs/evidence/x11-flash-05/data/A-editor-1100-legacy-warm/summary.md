# xflash analysis

Clock: A-editor-1100-legacy-warm: 11329 ext lines, 10708 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 0.792 ms, observer dropped 37340 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.17 ms wide; X11_OBS origin inside that interval (offset from its middle 1.16 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-text-editor | idle | large | 10 | 10 | 0 | 0 | 4 | 0 | 4 | 14.4 / 46.8 | 1.8 | 0/4 (10, 0) | - / - |
| gnome-text-editor | idle | small | 10 | 10 | 0 | 0 | 3 | 0 | 3 | 8.5 / 31.2 | 3.1 | 0/3 (10, 0) | - / - |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
