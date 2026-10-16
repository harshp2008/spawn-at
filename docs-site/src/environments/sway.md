# Sway

**Status: experimental, opt-in.** It is chosen only with `SPAWN_AT_BACKEND=sway`, never
automatically. **Tested only on a private headless Sway 1.9** with the project's own test window.
No real Sway session, no real application, no other Sway version.

It talks to Sway's IPC socket (`$SWAYSOCK`) directly. Nothing needs installing, and
`spawn-at install` has nothing to do for it.

```bash
# inside a Sway session ($SWAYSOCK is set by Sway)
SPAWN_AT_BACKEND=sway spawn-at spawn --anchor top-right -m 24 --size 600 400 -- foot

# in the Sway config
bindsym $mod+Shift+Return exec env SPAWN_AT_BACKEND=sway spawn-at spawn --anchor center -- foot
```

If `SPAWN_AT_BACKEND=sway` is set and `$SWAYSOCK` is missing, empty or not a Sway socket, spawn-at
stops with an error that names `$SWAYSOCK` and does not fall back to X11. Real output:

```text
[spawn-at] ERROR: Driver initialization error: IPC Error: SPAWN_AT_BACKEND=sway, but $SWAYSOCK is not set. Run spawn-at inside a Sway session, or unset SPAWN_AT_BACKEND.
```

The exit status is 1.

## What works

`spawn` with anchors, margins, `--size` and `--monitor <index>`; `query layout`, `query windows`,
`transform`, `close`, `focus`. Windows are made floating and placed with the same layout math as on
GNOME. `--size` is the size of the whole window including its border and title bar, as on GNOME. A
window cannot be made smaller than Sway lets it (75x50 for a floating window in the one setup tried);
it is then placed using the size it ended up with.

## What is different or missing

- **It can flash.** Sway has no way to hold a new window invisible until it is placed, so the window
  may show for one frame at its tiled position first. That one-frame flash is accepted for this
  release and **has not been measured**. The design says why a flicker-free placement cannot be
  promised: a command sent from another process may land after the first frame.
- **Unsupported commands:** `defocus`, `maximize`, `minimize`, `restore` and `query pointer` exit 1
  with a "does not support this feature" error and send nothing to Sway.
- **Unsupported options:** `--anchor cursor` and `--monitor cursor`. Sway has no pointer query, so
  `spawn` uses (0, 0) in place of the pointer position (`src/commands/spawn.rs` falls back to
  (0, 0) when the backend cannot read it).
- **`--monitor`** counts outputs in the order Sway lists them. Monitor 0 is reported as primary,
  because Sway has no primary output.
- **Matching the window.** A new window is matched to the launched process by pid, then by parent
  process, then by application id. If none appears within 5 seconds, spawn-at warns that the
  application was started but not placed, and moves nothing else. Windows that existed before the
  launch are never moved.
- **`--no-wait`** still waits for the placement. Only the first window of a batch is placed.
- A window in the scratchpad is listed with `state` `minimized`; `state` is otherwise `normal` or
  `fullscreen`.
- **i3 is not supported**, although it shares the IPC.
- `--area screen` and `--area unbounded` were not tried.

## What was run

- `scripts/sway-live.sh`: **62 checks, 0 failed** on 2026-10-15, in two private headless Sway
  sessions started by `scripts/sway-headless.sh`: one 1920x1080 output, then a scale-2 output with a
  30 px bar and a second output. Every position was compared with what `swaymsg` reports.
- Unit and integration tests against a scripted socket, with byte frames and JSON recorded from a
  real Sway 1.9.
- `scripts/verify-sway.sh`: the catalog specs for existing windows, **15 passed**; the 7 GNOME-only
  specs were Skipped.

## What was not run

A real Sway session or real applications; XWayland windows; Sway versions other than 1.9; i3; frame
timing before the first paint; `--area screen` and `--area unbounded`; clients that map at a size
other than the one asked; scales other than 1 and 2; more than two outputs; output names other than
`HEADLESS-*`.

## Sources

- `README.md` ("Sway (experimental)")
- `docs/DESIGN_SWAY.md`, sections 2, 4 to 7 and 12
- `src/commands/spawn.rs` (the cursor fallback; the design document's section 3 says
  `--anchor cursor` fails, its section 12 and the code say (0, 0))
- `docs/TEST_PLAN.md`, sections 7.1 to 7.5
- `docs/RELEASE_NOTES_0.2.0-beta.4.md` ("Sway: limitations")
- `docs/KNOWLEDGE.md` ("GNOME and Wayland behavior", "Gotchas")
- the error text above was produced by running the built binary with `SPAWN_AT_BACKEND=sway` and no `SWAYSOCK`
