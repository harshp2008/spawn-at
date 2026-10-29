# xflash analysis

Clock: rep-eog-large-2: 1059 ext lines, 728 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 0.787 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.41 ms wide; X11_OBS origin inside that interval (offset from its middle 1.67 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 5 | 5 | 0 | 5 | 4 | 0 | 5 | 24.1 / 24.6 | 2.5 | 1/5 (0, 4) | 18.9 / 19.3 |
| eog | idle | small | 5 | 5 | 0 | 4 | 1 | 0 | 4 | 10.2 / 28.1 | 3.3 | 3/4 (0, 1) | 18.2 / 18.5 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
