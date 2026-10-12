# Verify Fixtures

- `real_trace.log`: a real captured session/journal log. `tests/trace_analyser_test.rs::test_real_trace_drop_in_fixture` validates it when present.
- `real_placement_run.json`: a real `report.json` for `placement/bottom-right`. Recorded before the report environment had workarea fields, so it has none (it proves old recordings still load; `render` marks its geometry as extension-derived).
- `real_delay500_run.json`: a real `report.json` for `cloak/delayed-paint/delay-500`, recorded after the window-scoping fix and also without workarea fields. Its log slice contains a leftover window (`…468`) as well as the window named in stdout (`…469`).
- `real_delay500_probed_run.json`: a real `report.json` for `cloak/delayed-paint/delay-500` recorded with `environment.primary_workarea` (`query layout`, i.e. the extension) and `environment.independent_workarea` (mutter's `_NET_WORKAREA` via `xprop`), both `0,40 1920x1040` on the recording machine. `render --case delay-500@probed` therefore shows the independent geometry result.
- `real_delay0_no_trace_run.json`: a real `report.json` for `cloak/delayed-paint/delay-0` recorded with the current binary while the extension's logging was off and `gdbus` was not on `PATH`, so the timing log could not be enabled: empty log slice, `trace_note` set, status `pass-trace-unchecked`. `render --case delay-0@no-trace` shows it.

All `report.json` fixtures have the hostname replaced with `redacted`; everything else is exactly as recorded. They are embedded by `spawn-at-verify render` (see `src/tui/render.rs`), so edit them only by recording a new real run.

`SHA256SUMS` holds the hashes of the files above (`sha256sum -c SHA256SUMS` from this directory). `real_placement_run.json` was sanitised on 2026-10-12: its `environment.hostname` was the machine's name and is now `redacted`; no other byte changed. The Stage 1 files under `docs/evidence/stage1/` record the earlier, pre-sanitisation hash (`4f037bd3…`) and are left as they were.
