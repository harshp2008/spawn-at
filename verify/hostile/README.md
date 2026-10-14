# hostile/

Cells that run `spawn` against a deliberately badly behaved fixture window (harness-only flags
`--resize-delay-ms`, `--min-size`, `--max-size`, `--resize-increment`, `--fixed-size`,
`--stall-after-map-ms`, `--flap-ms`) and declare the ready-wait outcome they expect in
`[expect.resize]` (outcome, reason, `waited_ms` range, final size). The expectation is judged for
ready-wait runs only; a `--wait-strategy legacy` run reports no outcome and is not judged. Written
for the default cap of 250 ms (`DEFAULT_POLICY` in `assets/gnome/modern/ready_wait.js`). An
extension installed before the default was raised from 150 to 250 (`docs/DESIGN_READY_WAIT.md`
section 11) needs `--wait-strategy ready-wait --wait-cap-ms 250` to match. Results and the judgments: `docs/TEST_PLAN.md` 6.22.
