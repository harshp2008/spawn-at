# xflash analysis

Clock: X-eog-gate-cold: 579 ext lines, 402 X11_OBS clock lines, 4 windows with an origin, per-window origin spread max 0.250 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.17 ms wide; X11_OBS origin inside that interval (offset from its middle 1.53 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| eog | idle | large | 2 | 2 | 0 | 0 | 0 | 0 | 0 | -0.3 / -0.2 | 20.4 | 0/0 (0, 0) | 2.7 / 5.0 |
| eog | idle | small | 2 | 2 | 0 | 0 | 0 | 0 | 0 | -0.2 / -0.1 | 14.5 | 0/0 (0, 0) | 4.1 / 6.4 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
