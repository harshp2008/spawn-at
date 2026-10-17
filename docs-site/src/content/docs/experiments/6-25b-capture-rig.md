---
title: "6.25b Screen-capture rig"
---

Is there a prompt-free way to capture the screen, so flashes could be seen in pixels? (X11 investigation, stage 1, phases 1b, 1c and 2). No prompt-free capture method reached a frame, so the rig was not built.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.25b (the second part of section 6.25 of `docs/TEST_LOG.md`). |
| **Date** | 2026-10-14. |
| **Build** | Tag: none recorded. Tree: branch `refactor/v0.2.0-beta.1`, HEAD `b09e27f`, tree clean (the same baseline as [6.25a](/spawn-at/experiments/6-25a-window-ping/)). The output of `spawn-at --version` is not recorded for this section. |
| **Verdict** | No prompt-free capture method reached any frame, so phase 2 (the capture rig) was skipped. It was not built, and its calibration was not done. The frame interval and jitter of a capture were never measured. The log names what would unblock it: a consumer that can attach to the Mutter node on the default connection (unknown), or accepting one portal prompt (not done, by rule). |
| **Raw results** | `scratch/p26/` (gitignored). Not in the repository. |

## What was tested

**Question**

Which screen-capture method can run with no prompt to the user and deliver frames? The log's phase 2
would have built a capture rig on it: a fixture colour option, a frame classifier, and a comparison
of capture-visible flashes with the wire-time and stamp-time verdicts.

**Setup and environment**

Introspection and harmless calls only, on the GNOME session. Fixture only; nothing installed. The
log records the tools present locally: `pw-cli`, `pw-dump`, `pw-record` (audio),
`gst-launch-1.0` 1.24.2 with `pipewiresrc`, `ffmpeg`, `gnome-screenshot`, ImageMagick `import`,
`grim`, `gcc` and the libpipewire 1.0.5 headers. A `gjs` holder kept the Mutter session alive.

**Commands run**

Exact command lines not recorded. By the log's prose:

- one call to the D-Bus interface `org.gnome.Shell.Screenshot` (it wrote nothing);
- `CreateSession`, `RecordMonitor` and `Start` of `org.gnome.Mutter.ScreenCast` (v4), held open by a
  `gjs` script, then stopped;
- `gst-launch-1.0 pipewiresrc` with `path=<node id>`, `path=<object.serial>` and
  `target-object=<object.serial>` (three attempts);
- `pw-dump`, to see the node while the session lived.

The portal interfaces (`org.freedesktop.portal.ScreenCast` v5 and `.Screenshot`) were deliberately
not called, because the portal asks the user to choose what to share. That it asks is the log's
hypothesis ("standard behaviour").

## Result

| Method | Prompt-free? | Result |
| :-- | :-- | :-- |
| `org.gnome.Shell.Screenshot` | no | AccessDenied ("Screenshot is not allowed"); unusable, and one still image per call could not see a 16 ms flash anyway |
| Portal ScreenCast and Screenshot | no (hypothesis) | not called, by the no-prompt rule |
| `org.gnome.Mutter.ScreenCast` (v4) | yes | the session, the stream and a PipeWire node (`gnome-shell`, `Stream/Output/Video`) were created and stopped with no dialog on the D-Bus path; no consumer reached a frame |
| Local consumers | n/a | `grim` is a wlroots-protocol tool (hypothesis: not supported by Mutter, not run); `gnome-screenshot` and `import` are still-image tools |

The Mutter stream could not be consumed: all three `pipewiresrc` attempts ended with
`stream error: target not found` (the stream goes `connecting`, `paused`, then `error`, 2 ms later),
while `pw-dump` showed the node. The log's hypothesis is that the session manager does not link a
client on the default connection to this node. A native `pw_stream` client was not tried.

## Not verified

Whether a capture would agree with the wire-time and stamp-time verdicts. Whether a panel recording
indicator appeared (the log's author could not see the screen). The reason for the refusal.

## Sources

- `docs/TEST_LOG.md` section 6.25, "Phase 1b", "Phase 1c", "Phase 2" (lines 1370-1383, 1397)
- [Shortcomings](/spawn-at/shortcomings/) ("Limits of the flash measurement")
