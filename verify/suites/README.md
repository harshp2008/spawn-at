# Verification Suite Definitions

Purpose: Preset test suites for fast feedback and release gating (`quick.toml`, `full.toml`, `visual.toml`, `regression.toml`, `release-gate.toml`).

The `full` and `release-gate` suites include the `hostile` category (9 specs using the fixture's hostile flags). They need a live GNOME Wayland session with the fixture built, like the other automated categories. All 9 have `required = false`, so `release-gate` (`required_only = true`) lists none of them and its test count does not change; `full` goes from 36 to 45.
