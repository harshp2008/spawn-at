---
title: "6.40 The startup id spawn-at gives to launched programs"
---

A read-only audit of the `DESKTOP_STARTUP_ID` that spawn-at sets for every program it launches. Its `_TIME` number is not a valid X11 timestamp. That is established. Whether it harms anything is **not established**: the bisect ([6.41](/spawn-at/experiments/6-41-bisect-private-shell/)) saw no effect.

| Field | Value |
| :-- | :-- |
| **ID** | X11-FLASH-02f step 2. Follows [6.39](/spawn-at/experiments/6-39-private-headless-shell/). |
| **Date** | 2026-10-29. |
| **Build** | Source at `7c053b9` (spawn-at 0.2.4). Nothing was run for the audit itself. |
| **Verdict** | Specification mismatch confirmed from source; harm not shown. No fix proposed. |
| **Raw results** | The note in the repository (Evidence below). |

## At a glance

| Item | Finding | Basis |
| :-- | :-- | :-- |
| Value | `spawn-at-<pid>-<ms>_TIME<ms>`, ms since 1970, 13 digits | read from source |
| Set for | every program started through the GNOME or X11 driver, both `DESKTOP_STARTUP_ID` and `XDG_ACTIVATION_TOKEN`; the Sway driver sets neither | read from source |
| Used for | window matching: the CLI looks for it in title or class, the extension reads the window's startup id | read from source |
| Size | about 417 times 2^32; truncated to 32 bits it is 2260015965, not 0 | arithmetic |
| History | the `_TIME` suffix was added in `ccc84dd` to stop a GLib critical in GTK3 apps | git log |
| What GTK does with an oversized value | not known | second-hand only |

## What was tested

**Question**

Could the startup id make a GTK program on X11 send `_NET_ACTIVE_WINDOW` with a bad timestamp, which is the message seen before both crashes in [6.38](/spawn-at/experiments/6-38-shell-crash-live-xwayland-editor/)?

**Setup and environment**

Source reading and a web search for the specification. No application was launched.

## Result

An X11 timestamp is a 32-bit count of milliseconds since the X server started, with 0 meaning "current time". The startup-notification specification says the number after `_TIME` is the server timestamp of the user action that caused the launch (second-hand, from search snippets). GDK's documentation says a `_TIME` suffix is taken as the timestamp of the launching event, and a mirror of the GTK3 history shows it parsed with `strtoul` (second-hand). What current GTK4 does with a number that does not fit 32 bits was not read.

**Hypothesis, untested at the time of the audit:** GTK on X11 turns the oversized value into a timestamp Mutter treats as 0 and focus handling then goes wrong. It fits that xterm ignores the variable and never produced the warning. It does not fit that the truncated value is not 0. The cells that tested it are in 6.41.

The id also has a job, so a fix has to keep it unique and readable from the window's startup id: put a valid 32-bit value after `_TIME`, drop `_TIME` for X11 clients, or stop setting the variable for them and match another way. None is applied.

## Not verified

- Any effect of the value on any program.
- GTK4's current parsing code, the exact specification text (only snippets were read), Mutter's use of the user time.

## Sources

- [Audit note](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/startup-id-lead.md)
- `src/commands/spawn.rs` lines 276 to 283; `src/platform/linux/gnome/driver.rs` lines 317 to 318; `src/platform/linux/x11/mod.rs` lines 57 to 58; `src/commands/mod.rs` lines 208 to 253; `assets/gnome/modern/extension.js` line 385
- The links point into the repository on GitHub and resolve once the branch is merged.
