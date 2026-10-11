# Verify Fixtures

- `real_trace.log`: a real captured session/journal log. `tests/trace_analyser_test.rs::test_real_trace_drop_in_fixture` validates it when present.
- `real_placement_run.json`: a real `report.json` for `placement/bottom-right`.
- `real_delay500_run.json`: a real `report.json` for `cloak/delayed-paint/delay-500`, recorded after the window-scoping fix. Its log slice contains a leftover window (`…468`) as well as the window named in stdout (`…469`).

Both `report.json` fixtures have the hostname replaced with `redacted`; everything else is exactly as recorded. They are embedded by `spawn-at-verify render` (see `src/tui/render.rs`), so edit them only by recording a new real run.

`SHA256SUMS` holds the hashes of the files above (`sha256sum -c SHA256SUMS` from this directory). `real_placement_run.json` was sanitised on 2026-10-12: its `environment.hostname` was the machine's name and is now `redacted`; no other byte changed. The Stage 1 files under `docs/evidence/stage1/` record the earlier, pre-sanitisation hash (`4f037bd3…`) and are left as they were.
