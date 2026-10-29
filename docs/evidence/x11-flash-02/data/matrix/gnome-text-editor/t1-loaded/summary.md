# xflash analysis

Clock: t1-idle: 593 ext lines, 0 X11_OBS clock lines, 0 windows with an origin, per-window origin spread max 0.000 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 3.64 ms wide
t1-loaded: 605 ext lines, 0 X11_OBS clock lines, 0 windows with an origin, per-window origin spread max 0.000 ms, observer dropped 0 damage lines; SetLogging brackets (60 pairs, newest-first pairing): origin in an interval 17.42 ms wide

## X11/XWayland runs

| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |

## Native Wayland control runs (extension log only)

| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |
| gnome-text-editor | idle | large | 10 | 0 | - / - | 10x reached:at-target | 10 |
| gnome-text-editor | idle | small | 10 | 0 | - / - | 10x reached:at-target | 10 |
| gnome-text-editor | loaded | large | 10 | 0 | - / - | 10x reached:at-target | 10 |
| gnome-text-editor | loaded | small | 10 | 0 | - / - | 10x reached:at-target | 10 |
