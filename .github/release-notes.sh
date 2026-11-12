#!/bin/bash
# Release notes for a tag, built from commit subjects since the previous tag and grouped by gitmoji.
# Commits go straight to main here, so GitHub's own generated notes (which list pull requests) come out empty.
# Usage: .github/release-notes.sh v0.3.0
set -euo pipefail

tag=$1
prev=$(git describe --tags --abbrev=0 "$tag^" 2>/dev/null || true)
range=${prev:+$prev..}$tag

section() { # title, then the gitmoji that belong under it
  local title=$1 lines
  shift
  lines=$(git log --format=%s "$range" | while read -r emoji subject; do
    for e in "$@"; do
      if [ "$emoji" = "$e" ]; then echo "- $subject"; fi
    done
  done)
  if [ -n "$lines" ]; then printf '## %s\n\n%s\n\n' "$title" "$lines"; fi
}

section "New" ✨
section "Fixed" 🐛
# Highlights only: polish, security hardening, docs, CI and dependency bumps are left out.

if [ -n "$prev" ] && [ -n "${GITHUB_REPOSITORY:-}" ]; then
  echo "**Full changelog**: https://github.com/$GITHUB_REPOSITORY/compare/$prev...$tag"
fi
