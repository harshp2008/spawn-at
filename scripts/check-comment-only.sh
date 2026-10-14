#!/usr/bin/env bash
# scripts/check-comment-only.sh
# Thin wrapper around `cargo xtask check-comment-only`.
set -euo pipefail

cargo xtask check-comment-only "$@"
