# xflash analysis

Clock: rep-eog-large-1: 1018 ext lines, 686 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 3.948 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.01 ms wide; X11_OBS origin inside that interval (offset from its middle 1.47 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 5 | 5 | 0 | 5 | 4 | 0 | 5 | 22.8 / 23.9 | 2.5 | 2/5 (0, 3) | 18.2 / 18.5 |
| eog | idle | small | 5 | 5 | 0 | 4 | 2 | 0 | 5 | 14.9 / 42.7 | 3.0 | 3/5 (0, 2) | 18.8 / 21.6 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
