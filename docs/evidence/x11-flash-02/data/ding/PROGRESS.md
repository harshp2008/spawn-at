# Progress and Shell breaks (X11-FLASH-02i)

Every cell ran in its own fresh DING-only private headless Shell (`tools/ding-cell.sh`), which is
the rule after a crash. This file marks the breaks: each line is a private Shell that aborted with
`meta_window_unmanage: assertion failed (window->display->focus_window != window)`, and the next
cell got a new sandbox. The live Shell (pid 2718897, start time 81373971) was checked before and
after every cell and never changed (MEASURED; no `LIVE SHELL CHANGED` line in any runner log).

| Cell | Directory | Runs | Shell break |
| :-- | :-- | --: | :-- |
| f1_debug_focus | focus | 1 | **crash run 1** |
| f2_debug_plain_vars | focus | 1 | **crash run 1** |
| f3_debug_plain_vars_noding | focus | 1 | clean (no DING) |
| f4_debug_harmless_time123 | focus | 1 | clean |
| f5_debug_calc_x11 | focus | 1 | clean |
| g1_geom_none | factor | 1 | **crash run 1** |
| g2_geom_size | factor | 2 | **crash run 2** (run 1: no window, setup miss) |
| g3_geom_anchor | factor | 1 | **crash run 1** |
| g4_geom_both | factor | 1 | **crash run 1** |
| g5_no_wait (invalid) | factor | 5 | no crash, but `--no-wait --json` returns no window id, so nothing was closed; kept only to show why `--find-class` was added |
| g5b_no_wait | factor | 1 | **crash run 1** |
| g6_plain_armed_novars | factor | 5 | clean |
| g7_plain_armed_vars | factor | 1 | **crash run 1** |
| g8a_both_harmless_nosuffix | factor | 5 | clean |
| g8b_both_harmless_time123 | factor | 5 | clean |
| g8c_dsid_harmless_token_unset | factor | 5 | clean |
| g8d_token_harmless_dsid_unset | factor | 5 | clean |
| g8e_dsid_real_token_unset | factor | 1 | **crash run 1** |
| g8f_token_real_dsid_unset | factor | 1 | **crash run 1** |
| m_x11_eog | matrix-x11 | 5 | clean |
| m_x11_gnome-calculator | matrix-x11 | 5 | clean |
| m_x11_nautilus | matrix-x11 | 5 | clean (invalid: D-Bus service window, no variables) |
| m_x11_gnome-terminal | matrix-x11 | 5 | clean |
| m_x11_google-chrome (1st) | matrix-x11 | 5 | no window (sandbox refused) |
| m_x11_google-chrome (2nd) | matrix-x11 | 5 | clean |
| m_x11_xterm | matrix-x11 | 5 | no window (pty refused to the agent) |
| m_x11_java-swing | matrix-x11 | 5 | clean |
| m_x11_code | matrix-x11 | 5 | no window |
| m_x11_qt-assistant | matrix-x11 | 5 | clean |
| m_x11_firefox | matrix-x11 | 5 | no window (snap refuses the private runtime dir) |
| m_x11_editor_nowindow | matrix-x11 | 1 | **crash run 1** |
| m_wl_gnome-text-editor | matrix-wl | 5 | clean |
| m_wl_eog | matrix-wl | 5 | clean |
| m_wl_gnome-calculator | matrix-wl | 5 | clean |
| m_wl_nautilus | matrix-wl | 5 | clean |
| m_wl_gnome-terminal | matrix-wl | 5 | clean |
| m_wl_google-chrome | matrix-wl | 5 | clean |
| m_wl_qt-assistant | matrix-wl | 5 | clean |
| p1_patch_editor_x11 | mitigation | 5 | clean (patched binary) |
| p2_patch_eog_x11 | mitigation | 5 | clean (patched binary) |
| p3_patch_editor_wl | mitigation | 5 | clean (patched binary) |
| p4_patch_calc_x11 | mitigation | 5 | clean (patched binary) |

Eleven Shell breaks, all one assertion. The first attempt at f1 (15:08) failed before launching
anything: the cell tool was not built with `SPAWN_AT` support and the private Shell's PATH has no
`spawn-at`; it broke no Shell and produced no data (`data/ding/focus/ding-cells.log`, first block).
One earlier attempt at f2 (15:11) ran the plain cell while the extension's D-Bus object was not up
yet; the close still went through and the crash reproduced, and the runner now waits for the
extension before starting the cell.
