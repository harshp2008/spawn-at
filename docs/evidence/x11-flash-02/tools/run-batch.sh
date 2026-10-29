#!/bin/sh
# Thin wrapper: one xflash batch inside a memory-limited systemd scope (3 GB, no swap).
# Run from the repository root:  docs/evidence/x11-flash-02/tools/run-batch.sh --app xterm --out DIR ...
here=$(dirname "$0")
exec systemd-run --user --scope -p MemoryMax=3G -p MemorySwapMax=0 -- "$here/target/debug/xflash" run "$@"
