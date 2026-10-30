---
title: "6.23 Post-reinstall live check and XWayland signals"
---

Post-reinstall live check: default cap, A/D regression, XWayland signal study. The new default beats the legacy rule under load, and no XWayland signal is reliable enough to build on.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.23. Follows [6.22](/spawn-at/experiments/6-22-hostile-apps/); the texture signal is re-run in [6.24](/spawn-at/experiments/6-24-texture-reader/). |
| **Date** | 2026-10-13. |
| **Build** | A debug `spawn-at` newer than every source file. The log gives the stale-binary check but no single commit for the measurements; the draft of phase 1 had been committed first, and the first idle batch was blocked because the binary's embedded commit `6ebace0` did not match `HEAD` `1655116`. The version string is not recorded. Extension installed after a reinstall and a new login at 20:29 (gnome-shell pid 159346), protocol 6, installed files identical to `assets/gnome/modern`. No code changed. **A previous attempt of this check ended in a GNOME Shell crash.** The log records that Mutter 46.2 aborted at 20:28:39 with `libmutter:ERROR window.c:1518 meta_window_unmanage: assertion failed focus_window != window` after a real `gnome-text-editor` forced to X11 was closed while focused. The extension had only logged `REJECT_UNMATCHED` for that window. Nothing from the crashed run is reused: it was a different login (gnome-shell pid 116188), so every number below was measured again. Whether closing a focused XWayland window crashes Mutter 46.2 regardless of this extension was not tested. After that crash no real X11 or XWayland application was run. |
| **Verdict** | "Is any candidate reliable enough to build on? No." Product consequences unchanged: XWayland and X11 windows keep the legacy rule. |
| **Raw results** | `scratch/p23r/` (gitignored). Not in the repository. |

**Correction (2026-10-31).** The Build row above says it was not tested whether closing a focused
XWayland window crashes Mutter 46.2 regardless of this extension. It is now answered. The cause is the
long `_TIME` value in the startup id, fixed in PR #94. DING is not required: 2 of 2 old-id aborts
happened without it, and 50 of 50 closes with the fixed id were clean (run 03b,
`docs/evidence/x11-flash-03/findings.md`, on branch `exp/x11-flash-03`).

## What was tested

**Question**

(1) After a reinstall, is the default cap 250 live, and does a `cap_ms` in the config file behave as
documented? (2) Does the new default (arm D) behave better than the legacy rule (arm A)? (3) On the
XWayland fixture, which signals could tell that the client has painted at the new size?

**Setup and environment**

Native Wayland: specs `placement/bottom-right-smaller` and `bottom-right-larger`, arm order rotated
every iteration, one `run` per spec per arm, 1.5 s apart. Idle: 10 per arm per spec (40 runs). Loaded: 22
`xtask burn-cpu` threads after a 70 s warm-up (load average 19.65 after the warm-up, 25.9 at the end), 20
per arm per spec (80 runs). A is legacy by override; D is no override. Paint time is the wire commit.

XWayland: the fixture only, `--protocol x11`, `bottom-right-smaller` and `-larger`, 20 per spec idle and
20 per spec under the same burn (80 runs). They were direct `spawn-at spawn --json` calls with
`SPAWN_AT_FIXTURE_EVENT_DIR` set and `SetLogging(true)` for the batch, so that the extension's
`X11_OBS_*` times and the fixture's events share `CLOCK_MONOTONIC` without an anchor.

**Commands run**

`spawn-at-verify run`; `spawn-at spawn --json` with the fixture; `spawn-at config show`; `gdbus` for
`SetLogging` and `ProtocolVersion`; `xtask burn-cpu`. Full command lines not recorded.

## Result

Preflight: the harness with no override reported `wait_cap_ms 250` and `cap=250`; `config show` said
250 default; a temporary config `[wait] cap_ms = 400` was honoured by a direct spawn and ignored by the
harness.

| Load | Arm | Runs | Wire-time flash runs | Cloak median / p95 / max (ms) |
| :-- | :-: | --: | --: | :-- |
| idle | A | 20 | 0 | 19.6 / 34.9 / 51.1 |
| idle | D | 20 | 0 | 19.5 / 35.0 / 49.1 |
| loaded | A | 40 | 26 | 45.3 / 53.0 / 70.2 |
| loaded | D | 40 | 0 | 66.8 / 104.7 / 116.2 |

Loaded: D 0 of 40 against A 26 of 40 (Fisher two-sided p = 6e-11), at +21.5 ms of median cloak. No
`X11_OBS` line in any of the 120 native runs. The process finding: after every docs commit
`cargo build -p spawn-at` has to run before the next batch, or the preflight blocks it as a stale binary.

XWayland signal study (80 runs, all exit 0):

- A `damaged` event arrives before the client has painted at the final size in 59 of 80 runs (idle
  larger 20 of 20, loaded larger 20 of 20, loaded smaller 19 of 20, idle smaller 0 of 20).
- The first `damaged` does not depend on the client's painting: its time after the resize is the same in
  idle smaller and idle larger (median 5.3 and 5.5 ms) although the client's final paint is at 3.9 and
  19.9 ms. The log's hypothesis is that it is the damage of the extension's own move to the final
  position (not isolated by an experiment).
- No run had more than one `damaged` between the resize and the reveal. `first-frame` comes before the
  client's resize and paint in 80 of 80 runs, so it is not a paint signal.
- `texture=(na)` in 351 of 351 lines; the actor size and buffer rectangle equal the frame in 351 of 351.
  The log later corrected the texture reader (see 6.24).
- The legacy rule on XWayland revealed before the client's final paint in 6 of 20 idle larger runs, 18 of
  20 loaded smaller and 20 of 20 loaded larger.

| Candidate rule | Verdict in the log |
| :-- | :-- |
| (i) first `damaged` means "painted" | Not reliable: it marks the extension's own reposition |
| (ii) wait for a second `damaged` | Not usable on this evidence (0 of 80 had a second before the reveal) |
| (iii) `first-frame` | Not a paint signal |
| (iv) texture size equals the target | Could not be evaluated here; see 6.24 |
| (v) buffer rectangle or actor size | Not a paint signal (the window manager's rectangle) |

## Not verified

Native X11; any real X11 or XWayland application (none after the crash); Qt, GTK and Electron apps;
screen capture; whether a cloaked window gets `damaged` for its repaint (later answered in 6.26: it
does, but not the client's); the texture-size candidate; caps other than 250 (and 400 in the config
check); fractional scaling and more than one monitor; a release-build fixture; the cause of the Shell
crash of the earlier run.

## Sources

- `docs/TEST_LOG.md` section 6.23 (lines 1233-1319)
