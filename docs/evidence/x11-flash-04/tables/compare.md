
### Bad runs, capture mode

| group | n | bad (texture) before → after | bad (pixels) before → after | final texture never reached buffer, before → after |
| :-- | :-- | :-- | :-- | :-- |
| eog x11 1100x760 | 10 | 8 → 0 | 8 → 0 | 0 → 0 |
| eog x11 1100x760 | 20 | 16 → 0 | 16 → 0 | 0 → 0 |
| eog x11 600x420 | 20 | 0 → 0 | 0 → 0 | 0 → 0 |
| eog x11 1700x1000 | 20 | 17 → 0 | 17 → 0 | 0 → 0 |
| gnome-text-editor x11 1100x760 | 20 | 10 → 1 | 10 → 2 | 1 → 1 |
| gnome-text-editor x11 640x420 | 20 | 11 → 0 | 11 → 0 | 0 → 0 |
| gnome-calculator x11 500x400 | 20 | 18 → 0 | 18 → 0 | 0 → 0 |
| gnome-calculator x11 900x700 | 20 | 20 → 0 | 20 → 4 | 1 → 0 |
| xterm x11 400x260 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| xterm x11 900x560 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| java-swing x11 600x400 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| java-swing x11 900x650 | 10 | 1 → 0 | 1 → 0 | 0 → 0 |
| qt-assistant x11 1000x700 | 10 | 6 → 0 | 10 → 1 | 0 → 0 |
| qt-assistant x11 700x500 | 10 | 0 → 0 | 6 → 4 | 0 → 0 |
| google-chrome x11 1200x800 | 10 | 0 → 0 | 8 → 8 | 0 → 0 |
| google-chrome x11 700x480 | 10 | 0 → 0 | 10 → 10 | 0 → 0 |
| eog x11 1100x760 | 10 | 9 → 0 | 9 → 0 | 0 → 0 |
| gnome-text-editor wayland 1100x760 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| gnome-text-editor wayland 640x420 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| gnome-calculator wayland 500x400 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| gnome-calculator wayland 900x700 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| eog wayland 1100x760 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| eog wayland 600x420 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |

### Placement, capture and passive cells pooled by cell

| group | n | final = request | final = what the CLI reported | frame at the bottom-right anchor for its final size | ended 360x492 |
| :-- | :-- | :-- | :-- | :-- | :-- |
| eog x11 1100x760 | 20 | 20 → 20 | 20 → 20 | 20 → 20 | 0 → 0 |
| eog x11 600x420 | 20 | 20 → 20 | 20 → 20 | 20 → 20 | 0 → 0 |
| eog x11 1700x1000 | 20 | 20 → 20 | 20 → 20 | 20 → 20 | 0 → 0 |
| gnome-text-editor x11 1100x760 | 20 | 20 → 20 | 20 → 20 | 20 → 20 | 0 → 0 |
| gnome-text-editor x11 640x420 | 20 | 20 → 20 | 20 → 20 | 20 → 20 | 0 → 0 |
| gnome-calculator x11 500x400 | 20 | 0 → 0 | 20 → 20 | 20 → 20 | 4 → 6 |
| gnome-calculator x11 900x700 | 20 | 15 → 15 | 20 → 20 | 20 → 20 | 5 → 5 |
| gnome-calculator x11 500x400 | 20 | 0 → 0 | 20 → 20 | 20 → 20 | 7 → 4 |
| gnome-calculator x11 900x700 | 20 | 16 → 14 | 20 → 20 | 20 → 20 | 4 → 6 |
| xterm x11 400x260 | 10 | 0 → 0 | 10 → 10 | 10 → 10 | 0 → 0 |
| xterm x11 900x560 | 10 | 0 → 0 | 10 → 10 | 10 → 10 | 0 → 0 |
| java-swing x11 600x400 | 10 | 10 → 10 | 10 → 10 | 10 → 10 | 0 → 0 |
| java-swing x11 900x650 | 10 | 9 → 10 | 10 → 10 | 10 → 10 | 0 → 0 |
| qt-assistant x11 1000x700 | 10 | 10 → 10 | 10 → 10 | 10 → 10 | 0 → 0 |
| qt-assistant x11 700x500 | 10 | 10 → 10 | 10 → 10 | 10 → 10 | 0 → 0 |
| google-chrome x11 1200x800 | 10 | 10 → 10 | 10 → 10 | 10 → 10 | 0 → 0 |
| google-chrome x11 700x480 | 10 | 10 → 10 | 10 → 10 | 10 → 10 | 0 → 0 |

### Reveal delay and bad runs, passive cells

| group | n | bad (texture) | cloak→reveal median ms | p90 | max | gate held median / max ms (after) | gate reasons (after) |
| :-- | :-- | :-- | :-- | :-- | :-- | :-- | :-- |
| eog x11 1100x760 | 20 | 15 → 0 | 2.6 → 17.5 | 5.7 → 22.0 | 9.7 → 22.6 | 13.3 / 16.7 | texture-matched:20 |
| eog x11 600x420 | 20 | 1 → 0 | 2.4 → 8.5 | 6.0 → 12.4 | 9.8 → 13.3 | 6.3 / 10.8 | texture-matched:20 |
| eog x11 1700x1000 | 20 | 17 → 0 | 3.0 → 24.6 | 16.0 → 26.6 | 17.8 → 29.7 | 19.2 / 24.4 | texture-matched:20 |
| gnome-text-editor x11 1100x760 | 20 | 9 → 0 | 5.0 → 4.2 | 11.6 → 88.2 | 13.2 → 92.2 | 0.2 / 87.8 | texture-matched:20 |
| gnome-text-editor x11 640x420 | 20 | 10 → 0 | 4.9 → 35.6 | 7.4 → 76.4 | 8.9 → 86.0 | 31.4 / 82.0 | texture-matched:20 |
| gnome-calculator x11 500x400 | 20 | 18 → 0 | 36.2 → 86.5 | 37.0 → 95.3 | 48.8 → 114.7 | 47.5 / 93.6 | texture-matched:20 |
| gnome-calculator x11 900x700 | 20 | 20 → 0 | 7.3 → 82.0 | 10.7 → 93.5 | 19.9 → 101.3 | 78.3 / 97.5 | texture-matched:20 |
| xterm x11 400x260 | 10 | 0 → 0 | 32.4 → 32.3 | 38.8 → 41.5 | 42.5 → 43.0 | 0.1 / 0.1 | texture-matched:10 |
| xterm x11 900x560 | 10 | 0 → 0 | 32.3 → 32.7 | 33.8 → 35.3 | 40.8 → 41.9 | 0.0 / 0.1 | texture-matched:10 |
| java-swing x11 600x400 | 10 | 0 → 0 | 4.1 → 2.8 | 5.5 → 4.3 | 5.7 → 4.8 | 0.0 / 0.1 | texture-matched:10 |
| java-swing x11 900x650 | 10 | 2 → 0 | 3.3 → 12.6 | 7.5 → 14.5 | 8.5 → 14.6 | 9.1 / 10.3 | texture-matched:10 |
| qt-assistant x11 1000x700 | 10 | 4 → 0 | 3.7 → 11.6 | 9.9 → 15.5 | 10.5 → 27.6 | 9.1 / 18.4 | texture-matched:10 |
| qt-assistant x11 700x500 | 10 | 1 → 0 | 3.2 → 7.1 | 4.5 → 8.4 | 4.7 → 9.0 | 4.2 / 5.1 | texture-matched:10 |
| google-chrome x11 1200x800 | 10 | 0 → 0 | 11.5 → 14.8 | 18.5 → 25.6 | 19.7 → 28.7 | 10.5 / 24.9 | texture-matched:10 |
| google-chrome x11 700x480 | 10 | 0 → 0 | 5.5 → 14.0 | 17.7 → 20.7 | 24.7 → 23.8 | 9.4 / 15.9 | texture-matched:10 |

### Native Wayland control

| group | n | bad px | final=request | ready-wait elapsed median / max ms | cloak→reveal median / max ms |
| :-- | :-- | :-- | :-- | :-- | :-- |
| gnome-text-editor wayland 1100x760 | 10 | 0 → 0 | 10 → 10 | 7.9 / 11.1 → 7.2 / 11.3 | 9.5 / 12.7 → 8.7 / 12.9 |
| gnome-text-editor wayland 640x420 | 10 | 0 → 0 | 10 → 10 | 5.9 / 8.5 → 5.3 / 8.6 | 7.6 / 10.1 → 6.9 / 10.9 |
| gnome-calculator wayland 500x400 | 10 | 0 → 0 | 0 → 0 | 37.5 / 38.9 → 37.0 / 40.1 | 39.3 / 40.9 → 38.8 / 42.1 |
| gnome-calculator wayland 900x700 | 10 | 0 → 0 | 10 → 10 | 8.8 / 12.7 → 9.1 / 10.8 | 10.3 / 14.6 → 10.7 / 12.7 |
| eog wayland 1100x760 | 10 | 0 → 0 | 10 → 10 | 12.4 / 15.4 → 11.4 / 15.2 | 13.9 / 17.0 → 12.8 / 17.1 |
| eog wayland 600x420 | 10 | 0 → 0 | 10 → 10 | 7.6 / 11.1 → 7.7 / 10.4 | 8.8 / 12.6 → 8.9 / 12.5 |

### Same-Shell checks (AFTER extension only)

| cell | group | n | bad texture | bad px | gate held med / max ms | reasons |
| :-- | :-- | --: | --: | --: | :-- | :-- |
| aft-off-eog-1100 | eog x11 1100x760 | 20 | 14 | 14 | - / - | - |
| aft-off-editor | gnome-text-editor x11 640x420 | 10 | 4 | 4 | - / - | - |
| aft-off-editor | gnome-text-editor x11 1100x760 | 10 | 5 | 5 | - / - | - |
| aft-capdemo-eog-1100 | eog x11 1100x760 | 20 | 0 | 0 | 9.4 / 12.3 | cap:13 texture-matched:7 |
| aft-capdemo-eog-1700 | eog x11 1700x1000 | 10 | 7 | 7 | 6.0 / 12.4 | cap:10 |
