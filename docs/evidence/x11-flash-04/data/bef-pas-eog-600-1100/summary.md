# xflash analysis

Clock: bef-pas-eog-600-1100: 6853 ext lines, 5712 X11_OBS clock lines, 40 windows with an origin, per-window origin spread max 0.509 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.45 ms wide; X11_OBS origin inside that interval (offset from its middle 1.16 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 20 | 20 | 0 | 17 | 3 | 0 | 18 | 13.4 / 19.4 | 2.5 | 16/18 (0, 2) | 18.2 / 19.1 |
| eog | idle | small | 20 | 20 | 0 | 2 | 0 | 0 | 2 | 5.6 / 10.6 | 2.3 | 2/2 (0, 0) | 16.2 / 19.8 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
