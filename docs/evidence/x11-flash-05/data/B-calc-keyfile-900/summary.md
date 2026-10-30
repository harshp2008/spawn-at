# xflash analysis

Clock: B-calc-keyfile-900: 11125 ext lines, 10484 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 3.583 ms, observer dropped 6717 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.23 ms wide; X11_OBS origin inside that interval (offset from its middle 1.15 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-calculator-keyfile | idle | large | 10 | 10 | 0 | 0 | 0 | 0 | 0 | -79.9 / -66.7 | 84.5 | 0/0 (0, 0) | -63.2 / -50.0 |
| gnome-calculator-keyfile | idle | small | 10 | 10 | 0 | 0 | 0 | 0 | 0 | -73.5 / -65.5 | 78.4 | 0/0 (0, 0) | -56.8 / -48.8 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
