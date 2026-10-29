# xflash analysis

Clock: aft-pas-editor: 49401 ext lines, 48180 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 1.208 ms, observer dropped 73445 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.29 ms wide; X11_OBS origin inside that interval (offset from its middle 1.18 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-text-editor | idle | large | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -0.8 / 0.6 | 4.1 | 0/0 (11, 0) | -54.5 / -45.2 |
| gnome-text-editor | idle | small | 20 | 20 | 0 | 0 | 0 | 0 | 0 | -63.5 / -0.2 | 35.6 | 0/0 (10, 0) | -55.1 / -46.8 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
