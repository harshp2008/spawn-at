#!/usr/bin/env bash
# scripts/check-doc-coverage.sh
# Thin wrapper around `cargo xtask check-doc-coverage`.
set -euo pipefail

cargo xtask check-doc-coverage "$@"
