# X11-FLASH-02i-b calibration: live xterm vs private headless xterm

Private headless Shell, DING not loaded. Both xterm conditions use the same batch options as the
live batches (`--app xterm --cond idle --reps 20 --seed 11` and `--cond loaded --reps 20 --seed 12`,
22 burn threads, 30 s warm-up; `run.rs` sizes 400x260 and 900x560). Live numbers re-printed from
`scratch/xflash02/b_xterm_{idle,loaded}` (the committed copies are `data/xterm-live/`); private data
in `data/matrix/xterm/{idle,loaded}`. The private Shell was the one private headless Shell of this
run; the live Shell pid 2718897 / start 81373971 did not change before or after either batch.

| condition | size | source | n | V | R | D | A | L med ms | L max ms |
| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: |
| idle | large | live | 20 | 0 | 0 | 0 | 0 | -10.5 | -4.0 |
| idle | large | private | 20 | 0 | 0 | 0 | 0 | -0.1 | 1.6 |
| idle | small | live | 20 | 0 | 0 | 1 | 0 | -11.0 | 36.6 |
| idle | small | private | 20 | 0 | 0 | 0 | 0 | -0.5 | -0.2 |
| loaded | large | live | 20 | 0 | 4 | 6 | 0 | 2.4 | 88.2 |
| loaded | large | private | 20 | 0 | 4 | 5 | 0 | -2.9 | 308.9 |
| loaded | small | live | 20 | 0 | 6 | 6 | 0 | -7.5 | 96.5 |
| loaded | small | private | 20 | 0 | 2 | 1 | 0 | -5.8 | 168.2 |

The counts are similar in three of the four cells and differ in the loaded small cell.

## Raw data

- Private: `data/matrix/xterm/idle/` and `data/matrix/xterm/loaded/` (runs.jsonl, runs.csv,
  summary.md, progress.jsonl, ext.log, calib.jsonl, xobs.jsonl, end.json, meta.json).
- Live: `data/xterm-live/idle/` and `data/xterm-live/loaded/` (summary only); the full live batch
  directories stayed in the git-ignored `scratch/xflash02/` and were only re-analysed, not copied.
