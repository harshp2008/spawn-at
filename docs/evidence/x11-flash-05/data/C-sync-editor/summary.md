# xflash analysis

Clock: C-sync-editor: 1301 ext lines, 1182 X11_OBS clock lines, 2 windows with an origin, per-window origin spread max 0.861 ms, observer dropped 1940 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.06 ms wide; X11_OBS origin inside that interval (offset from its middle 1.04 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-text-editor | idle | large | 1 | 1 | 0 | 0 | 0 | 0 | 0 | -0.3 / -0.3 | 3.3 | 0/0 (1, 0) | - / - |
| gnome-text-editor | idle | small | 1 | 1 | 0 | 0 | 0 | 0 | 0 | -0.1 / -0.1 | 2.9 | 0/0 (1, 0) | - / - |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
