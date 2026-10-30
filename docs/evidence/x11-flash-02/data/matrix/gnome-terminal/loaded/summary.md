# xflash analysis

Clock: idle: 1461 ext lines, 0 X11_OBS clock lines, 0 windows with an origin, per-window origin spread max 0.000 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 4.01 ms wide
loaded: 1181 ext lines, 0 X11_OBS clock lines, 0 windows with an origin, per-window origin spread max 0.000 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 11.75 ms wide

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |
| gnome-terminal | idle | large | 20 | 0 | 0 | 0 | 0 | 0 | 0 | - / - | - | 0/0 (0, 0) | - / - |
| gnome-terminal | idle | small | 20 | 0 | 0 | 0 | 0 | 0 | 0 | - / - | - | 0/0 (0, 0) | - / - |
| gnome-terminal | loaded | large | 20 | 0 | 0 | 0 | 0 | 0 | 0 | - / - | - | 0/0 (0, 0) | - / - |
| gnome-terminal | loaded | small | 20 | 0 | 0 | 0 | 0 | 0 | 0 | - / - | - | 0/0 (0, 0) | - / - |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
| gnome-terminal | idle | large | 5 | 0 | - / - | 5x adjusted:quiet | 0 |
| gnome-terminal | idle | small | 5 | 0 | - / - | 5x adjusted:quiet | 0 |
