#!/usr/bin/env bash
# Usage: ./scripts/bump-version.sh [patch|minor|major|x.y.z]
# Updates the version in all three places that need it:
#   - apps/desktop/src-tauri/tauri.conf.json
#   - apps/desktop/src-tauri/Cargo.toml
#   - apps/desktop/package.json

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

TAURI_CONF="$REPO_ROOT/apps/desktop/src-tauri/tauri.conf.json"
CARGO_TOML="$REPO_ROOT/apps/desktop/src-tauri/Cargo.toml"
PACKAGE_JSON="$REPO_ROOT/apps/desktop/package.json"

# Read current version from tauri.conf.json (authoritative source).
CURRENT=$(grep '"version"' "$TAURI_CONF" | head -1 | sed 's/.*"version": *"\([^"]*\)".*/\1/')

IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

ARG="${1:-patch}"

case "$ARG" in
  major)
    NEW="$((MAJOR + 1)).0.0"
    ;;
  minor)
    NEW="$MAJOR.$((MINOR + 1)).0"
    ;;
  patch)
    NEW="$MAJOR.$MINOR.$((PATCH + 1))"
    ;;
  [0-9]*.[0-9]*.[0-9]*)
    NEW="$ARG"
    ;;
  *)
    echo "Usage: $0 [patch|minor|major|x.y.z]" >&2
    exit 1
    ;;
esac

echo "Bumping $CURRENT → $NEW"

# tauri.conf.json
sed -i "s/\"version\": \"$CURRENT\"/\"version\": \"$NEW\"/" "$TAURI_CONF"

# Cargo.toml — only the [package] version line (first occurrence)
sed -i "0,/^version = \"$CURRENT\"/{s/^version = \"$CURRENT\"/version = \"$NEW\"/}" "$CARGO_TOML"

# package.json
sed -i "s/\"version\": \"$CURRENT\"/\"version\": \"$NEW\"/" "$PACKAGE_JSON"

echo "Updated:"
echo "  $TAURI_CONF"
echo "  $CARGO_TOML"
echo "  $PACKAGE_JSON"
echo ""
echo "Next steps:"
echo "  git add -p"
echo "  git commit -m \"chore: bump version to $NEW\""
echo "  git tag v$NEW"
echo "  git push && git push --tags"
