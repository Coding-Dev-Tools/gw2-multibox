#!/usr/bin/env bash
# verify-sha-pins.sh — CI regression guard for supply-chain integrity
# Fails if any workflow uses a mutable action tag (@v1, @v2, @main, etc.)
# instead of a 40-character SHA pin.
#
# Usage: bash scripts/verify-sha-pins.sh
# Exit 0 = all actions are SHA-pinned
# Exit 1 = one or more mutable tags found

set -euo pipefail

WORKFLOW_DIR=".github/workflows"

if [ ! -d "$WORKFLOW_DIR" ]; then
  echo "ERROR: $WORKFLOW_DIR not found"
  exit 1
fi

VIOLATIONS=0

# Match 'uses:' lines that do NOT end with a 40-char hex SHA before any comment
# Valid:   uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.2.2
# Invalid: uses: actions/checkout@v4
# Invalid: uses: actions/checkout@main
# Exempt:  org-internal reusable workflows (same-org .github repo @main)

while IFS= read -r file; do
  while IFS= read -r line; do
    # Extract the uses: value
    uses_value=$(echo "$line" | sed -n 's/.*uses:[[:space:]]*//p' | sed 's/[[:space:]]*#.*//' | tr -d '[:space:]')
    
    if [ -z "$uses_value" ]; then
      continue
    fi

    # Skip local actions (./path)
    if echo "$uses_value" | grep -q '^\./'; then
      continue
    fi

    # Check if it has @ followed by exactly 40 hex chars
    if echo "$uses_value" | grep -qE '@[0-9a-f]{40}$'; then
      continue
    fi

    # Exempt org-internal reusable workflows (Coding-Dev-Tools/.github/...@main)
    if echo "$uses_value" | grep -qE '^Coding-Dev-Tools/\.github/.*@main$'; then
      continue
    fi

    echo "VIOLATION: $file: mutable tag found: $uses_value"
    VIOLATIONS=$((VIOLATIONS + 1))
  done < <(grep -n 'uses:' "$file" | grep -v '^[[:space:]]*#')
done < <(find "$WORKFLOW_DIR" -name '*.yml' -o -name '*.yaml')

if [ "$VIOLATIONS" -gt 0 ]; then
  echo ""
  echo "FAILED: $VIOLATIONS mutable action tag(s) found."
  echo "All third-party actions must be SHA-pinned to 40-character commit hashes."
  echo "Example: uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.2.2"
  exit 1
fi

echo "OK: All workflow actions are SHA-pinned."
exit 0
