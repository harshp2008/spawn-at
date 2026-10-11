#!/usr/bin/env bash
# scripts/check-doc-coverage.sh
# Verifies that every .rs file in crates/spawn-at-verify/src and tests has:
# 1. A //! file-level header doc comment.
# 2. A preceding /// doc comment for every pub/pub(crate) item and every fn (including tests).

set -euo pipefail

TARGET_DIRS="${@:-crates/spawn-at-verify/src crates/spawn-at-verify/tests}"

python3 - <<EOF
import os, sys, re, glob

target_dirs = "$TARGET_DIRS".split()
rs_files = []
for d in target_dirs:
    if os.path.isfile(d) and d.endswith('.rs'):
        rs_files.append(d)
    elif os.path.isdir(d):
        for root, _, files in os.walk(d):
            for f in sorted(files):
                if f.endswith('.rs'):
                    rs_files.append(os.path.join(root, f))

rs_files.sort()

# Item regex:
# Matches:
# 1) Any fn: (pub(...) )? (async )? (unsafe )? fn name
# 2) Any pub/pub(...) struct, enum, trait, type, const, static
fn_pattern = re.compile(r'^\s*(?:pub(?:\s*\([^)]+\))?\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+([a-zA-Z0-9_]+)')
pub_item_pattern = re.compile(r'^\s*pub(?:\s*\([^)]+\))?\s+(?:struct|enum|trait|type|const|static)\s+([a-zA-Z0-9_]+)')

total_misses = 0
file_misses = {}

for file_path in rs_files:
    with open(file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    misses = []

    # 1. Check for //! header
    has_header = False
    for line in lines:
        s = line.strip()
        if not s:
            continue
        if s.startswith('//!'):
            has_header = True
            break
        # If we hit an item, code, or non-inner-doc before //!, it lacks //!
        if not s.startswith('//'):
            break

    if not has_header:
        misses.append((1, "missing //! file header doc comment"))

    # 2. Check items
    in_attr = False
    for i, raw_line in enumerate(lines):
        line = raw_line.strip()
        if not line or line.startswith('//') or line.startswith('/*'):
            continue

        match_fn = fn_pattern.match(raw_line)
        match_pub = pub_item_pattern.match(raw_line)

        target_name = None
        item_kind = None
        if match_fn:
            target_name = match_fn.group(1)
            item_kind = "fn"
        elif match_pub:
            target_name = match_pub.group(1)
            item_kind = "pub item"

        if target_name:
            # Look backwards past attributes (#[...]) and blank lines
            j = i - 1
            has_doc = False
            inside_attr = False
            while j >= 0:
                prev = lines[j].strip()
                if not prev:
                    j -= 1
                    continue
                if prev.endswith(']'):
                    inside_attr = True
                if inside_attr:
                    if prev.startswith('#[') or prev.startswith('#!['):
                        inside_attr = False
                    j -= 1
                    continue
                if prev.startswith('///'):
                    has_doc = True
                    break
                # Hit something else (code, // non-doc comment, etc.)
                break

            if not has_doc:
                misses.append((i + 1, f"missing /// doc for {item_kind} '{target_name}'"))

    if misses:
        total_misses += len(misses)
        file_misses[file_path] = misses

if total_misses > 0:
    print(f"FAILED: Found {total_misses} documentation misses across {len(file_misses)} files:\n")
    for fp, misses in sorted(file_misses.items()):
        print(f"=== {fp} ({len(misses)} misses) ===")
        for line_no, desc in misses:
            print(f"  Line {line_no:4d}: {desc}")
        print()
    sys.exit(1)
else:
    print(f"SUCCESS: ZERO documentation misses across all {len(rs_files)} files!")
    sys.exit(0)
EOF
