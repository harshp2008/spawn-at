# xflash analysis

Clock: sweep-eog-600: 1171 ext lines, 839 X11_OBS clock lines, 10 windows with an origin, per-window origin spread max 3.931 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.11 ms wide; X11_OBS origin inside that interval (offset from its middle 1.56 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 5 | 5 | 0 | 0 | 0 | 0 | 0 | 4.7 / 7.1 | 7.2 | 0/0 (0, 0) | 11.7 / 16.1 |
| eog | idle | small | 5 | 5 | 0 | 0 | 2 | 0 | 2 | 9.1 / 36.5 | 5.3 | 0/2 (0, 2) | 16.1 / 16.3 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
