# X11-FLASH-02i-b progress (private headless Shell, DING not loaded)

Live gnome-shell pid 2718897, start time 81373971 was recorded before every batch and after every
batch of this run.

| time | event |
| :-- | :-- |
| 16:09 | private Shell started, sandbox `/tmp/xflash-priv.wLvpdv`; active extensions `['spawn-at@harsh.local']`, DING installed but disabled (state 6) |
| 16:10 | smoke test xterm, 3 of 3 runs ok; live pid unchanged |
| 16:14 | xterm idle 40 of 40 runs ok, no crash; live pid unchanged |
| 16:20 | xterm loaded 40 of 40 runs ok, no crash; live pid unchanged |
| 16:26 | gnome-text-editor idle 50 of 50 runs ok, no crash; live pid unchanged |
| 16:27 | gnome-text-editor loaded run 0 no_window, batch stopped on a late window (not a Shell crash); window closed, batch restarted |
| 16:34 | gnome-text-editor loaded 40 of 40 runs ok, no crash; live pid unchanged |
| 16:39 | eog idle 50 of 50 runs ok, no crash; live pid unchanged |
| 16:40 | eog loaded run 0 no_window (large), batch stopped on the late window (not a Shell crash); window closed, batch resumed; 39 of 40 runs in total, loaded large valid n=19 |
| 16:46 | eog loaded finished, no crash; live pid unchanged |
| 16:52 | gnome-calculator idle 50 of 50 runs ok, no crash; live pid unchanged |
| 16:53 | gnome-calculator loaded run 0 and run 1 no_window under the 22-thread load (default spawn window timeout 4 s, window appeared at ~5 s); windows closed, partial batch removed |
| 16:56 | spawn-at config `$SB/none.toml` raised `spawn.window_timeout_ms` to 15000 for this sandbox only; calculator loaded restarted, run 0 ok |
| 17:01 | gnome-calculator loaded 40 of 40 runs ok, no crash; live pid unchanged |
| 17:08 | gnome-terminal idle 50 of 50 runs ok, no crash; live pid unchanged; every window was `proto=Wayland` class `gnome-terminal-server` (the server decides the backend), so the X11 rows have valid 0 |
| 17:14 | gnome-terminal loaded 40 of 40 runs ok, no crash; live pid unchanged |
| 17:21 | google-chrome idle 50 of 50 runs ok, no crash; live pid unchanged |
| 17:28 | google-chrome loaded 40 of 40 runs ok, no crash; live pid unchanged |
| 17:34 | qt-assistant idle 50 of 50 runs ok, no crash; live pid unchanged |
| 17:41 | qt-assistant loaded 40 of 40 runs ok, no crash; live pid unchanged |
| 17:49 | java-swing idle 40 of 40 runs ok, no crash; live pid unchanged |
| 17:59 | java-swing loaded 40 of 40 runs ok, no crash; live pid unchanged |
| 18:04 | gnome-text-editor-nowindow idle 50 of 50 runs ok, no crash; live pid unchanged |
| 18:10 | gnome-text-editor-nowindow loaded 40 of 40 runs ok, no crash; live pid unchanged |
| 18:38 | T1 native Wayland batches done for gnome-text-editor, eog, google-chrome, qt-assistant (20 runs each per condition, idle and loaded); no crash; live pid unchanged |
| 18:42 | T2 plain geometry runs: xterm 10 of 10, google-chrome 10 of 10; Qt Assistant skipped (`Unknown option: -geometry`, exit 255) |
| 18:45 | pixel capture: attempt 1 (introspect) ok; attempt 2 failed with `Failed to start screen cast: Couldn't connect pipewire context`; attempt 3 not run per the time-box; private Shell still running, live pid unchanged |
