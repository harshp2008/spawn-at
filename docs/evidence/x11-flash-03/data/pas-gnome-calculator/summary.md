# xflash analysis

Clock: pas-gnome-calculator: 18895 ext lines, 18259 X11_OBS clock lines, 20 windows with an origin, per-window origin spread max 3.488 ms, observer dropped 17390 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.14 ms wide; X11_OBS origin inside that interval (offset from its middle 1.62 ms)

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-calculator | idle | large | 10 | 10 | 0 | 5 | 0 | 2 | 5 | 1.4 / 4.2 | 6.8 | 5/5 (0, 0) | 15.2 / 20.0 |
| gnome-calculator | idle | small | 10 | 10 | 0 | 1 | 3 | 1 | 4 | -0.2 / 54.8 | 36.9 | 1/4 (0, 3) | -17.7 / 15.4 |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
