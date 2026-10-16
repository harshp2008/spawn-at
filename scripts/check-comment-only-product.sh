#!/usr/bin/env bash
# scripts/check-comment-only-product.sh <git-rev>
# Thin wrapper around `cargo xtask check-comment-only`: proves that everything in the product
# code (the binary, build.rs, the root tests, spawn-at-core, spawn-at-verify) differs from <git-rev>
# in comments only. crates/xtask is left out: it is repository tooling and may gain code.
set -euo pipefail

rev="${1:?usage: scripts/check-comment-only-product.sh <git-rev>}"
for path in src build.rs tests crates/spawn-at-core crates/spawn-at-verify; do
  cargo xtask check-comment-only "$rev" "$path"
done
