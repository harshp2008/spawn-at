# hostile/

Cells that run `spawn` against a deliberately badly behaved fixture window (harness-only flags
`--resize-delay-ms`, `--min-size`, `--max-size`, `--resize-increment`, `--fixed-size`,
`--stall-after-map-ms`, `--flap-ms`) and declare the ready-wait outcome they expect in
`[expect.resize]` (outcome, reason, `waited_ms` range, final size). The expectation is judged for
ready-wait runs only; a `--wait-strategy legacy` run reports no outcome and is not judged. Written
for the default cap of 250 ms: run with `--wait-strategy ready-wait --wait-cap-ms 250` on an
extension whose default cap is 150. Results and the judgments: `docs/TEST_PLAN.md` 6.22.
