#!/usr/bin/env bash
# scripts/check-comment-only.sh
# Verifies that changes between a base commit and current state contain ONLY comments and whitespace.
# Usage: ./scripts/check-comment-only.sh <base-commit> [path-prefix]

set -euo pipefail

BASE_COMMIT="${1:-HEAD~1}"
TARGET_DIR="${2:-crates/spawn-at-verify}"

echo "==> Checking that changes since $BASE_COMMIT in $TARGET_DIR are comment-only..."

# Find all changed .rs files in target dir
CHANGED_FILES=$(git diff --name-only "$BASE_COMMIT" -- "$TARGET_DIR" | grep '\.rs$' || true)

if [ -z "$CHANGED_FILES" ]; then
    echo "No .rs files changed in $TARGET_DIR compared to $BASE_COMMIT."
    exit 0
fi

FAILED=0

python3 - <<EOF
import sys, subprocess, difflib, os

base_commit = "$BASE_COMMIT"
files = """$CHANGED_FILES""".strip().splitlines()
has_failure = False

def strip_comments_and_fmt(text):
    out = []
    in_block = False
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        # Check block comment delimiters
        if in_block:
            if '*/' in line:
                idx = line.find('*/')
                line = line[idx+2:].strip()
                in_block = False
            else:
                continue
        if line.startswith('/*'):
            if '*/' in line:
                idx = line.find('*/')
                line = line[idx+2:].strip()
            else:
                in_block = True
                continue
        if line.startswith('//'):
            continue
        # Strip trailing // comments if not inside strings
        if '//' in line:
            # Simple check: find first // not in quotes
            in_str = False
            comment_start = -1
            for i, ch in enumerate(line):
                if ch == '"' and (i == 0 or line[i-1] != '\\\\'):
                    in_str = not in_str
                elif not in_str and ch == '/' and i + 1 < len(line) and line[i+1] == '/':
                    comment_start = i
                    break
            if comment_start != -1:
                line = line[:comment_start].strip()
        if line:
            # normalize spaces
            out.append(' '.join(line.split()))
    return out

for file_path in files:
    # Get base commit version
    res_base = subprocess.run(['git', 'show', f'{base_commit}:{file_path}'], capture_output=True, text=True)
    if res_base.returncode != 0:
        print(f"Skipping newly created file or git error on {file_path}")
        continue
    base_content = res_base.stdout
    
    # Get current version (from disk if exists, else HEAD)
    if os.path.exists(file_path):
        with open(file_path, 'r', encoding='utf-8') as f:
            curr_content = f.read()
    else:
        res_curr = subprocess.run(['git', 'show', f'HEAD:{file_path}'], capture_output=True, text=True)
        curr_content = res_curr.stdout

    base_stripped = strip_comments_and_fmt(base_content)
    curr_stripped = strip_comments_and_fmt(curr_content)

    diff = list(difflib.unified_diff(
        base_stripped, curr_stripped,
        fromfile=f'base:{file_path}', tofile=f'current:{file_path}',
        lineterm=''
    ))

    if diff:
        print(f"FAILED: Non-comment change detected in {file_path}:")
        for d in diff[:25]:
            print("  ", d)
        if len(diff) > 25:
            print(f"   ... and {len(diff) - 25} more diff lines")
        has_failure = True
    else:
        print(f"  OK (comments only): {file_path}")

if has_failure:
    sys.exit(1)
else:
    print("ALL checked files contain ONLY comment changes!")
EOF

