#!/usr/bin/env bash
set -euo pipefail

# Fully automated release for Nodepad (Tauri 2 macOS app).
#
# Usage: ./scripts/release.sh v0.1.0
#
# What it does:
#   1. Validates the version argument and git state.
#   2. Runs the full test matrix (npm run check).
#   3. Generates CHANGELOG.md from git log.
#   4. Bumps version in package.json, tauri.conf.json, Cargo.toml, and Cargo.lock.
#   5. Commits, tags, and pushes.
#   6. Builds the unsigned macOS DMG with updater artifacts.
#   7. Generates latest.json for the Tauri in-app updater.
#   8. Creates a GitHub release with all artifacts.
#
# Options:
#   --skip-tests     Skip the test matrix (use only when you already ran it).
#   --allow-dirty    Allow a non-clean git working tree.
#   --any-branch     Allow running from a branch other than main.
#   --dry-run        Run tests and print the plan, but do not publish anything.

VERSION=""
SKIP_TESTS=false
ALLOW_DIRTY=false
ANY_BRANCH=false
DRY_RUN=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-tests)
      SKIP_TESTS=true
      shift
      ;;
    --allow-dirty)
      ALLOW_DIRTY=true
      shift
      ;;
    --any-branch)
      ANY_BRANCH=true
      shift
      ;;
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    -*)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
    *)
      if [[ -n "$VERSION" ]]; then
        echo "Only one version argument allowed." >&2
        exit 1
      fi
      VERSION="$1"
      shift
      ;;
  esac
done

if [[ -z "$VERSION" ]]; then
  echo "Usage: release.sh <tag>  e.g. v0.1.0" >&2
  echo "Options: --skip-tests --allow-dirty --any-branch --dry-run" >&2
  exit 1
fi

if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Invalid version: $VERSION (expected vX.Y.Z)" >&2
  exit 1
fi
TAG_VERSION="${VERSION#v}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# ─── Git state checks ────────────────────────────────────────────────────

CURRENT_BRANCH="$(git branch --show-current)"
if [[ "$CURRENT_BRANCH" != "main" ]] && [[ "$ANY_BRANCH" != true ]]; then
  echo "Not on main branch (currently on $CURRENT_BRANCH). Use --any-branch to override." >&2
  exit 1
fi

if [[ -n "$(git status --short)" ]] && [[ "$ALLOW_DIRTY" != true ]]; then
  echo "Working tree is not clean. Commit or stash changes, or use --allow-dirty." >&2
  git status --short >&2
  exit 1
fi

if git rev-parse "$VERSION" >/dev/null 2>&1; then
  echo "Tag $VERSION already exists." >&2
  exit 1
fi

# ─── Test matrix ──────────────────────────────────────────────────────────

if [[ "$SKIP_TESTS" != true ]]; then
  echo "→ Running test matrix (npm run check)..."
  npm run check
else
  echo "→ Skipping tests (as requested)."
fi

# ─── Changelog ────────────────────────────────────────────────────────────

echo "→ Generating changelog from git log..."
node scripts/generate-changelog.mjs "--tag=$VERSION"

# ─── Dry-run stop ────────────────────────────────────────────────────────

if [[ "$DRY_RUN" == true ]]; then
  echo "→ Dry-run: would bump version to $TAG_VERSION"
  echo "→ Dry-run: would commit, tag $VERSION, and push"
  echo "→ Dry-run: would build macOS DMG with updater artifacts"
  echo "→ Dry-run: would generate latest.json"
  echo "→ Dry-run: would create GitHub release"
  exit 0
fi

# ─── Version bump ─────────────────────────────────────────────────────────

echo "→ Bumping version to $TAG_VERSION..."
node scripts/update-version.mjs "$TAG_VERSION"

# ─── Commit, tag, push ───────────────────────────────────────────────────

echo "→ Committing release..."
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock CHANGELOG.md
git commit -m "Release $VERSION"
git tag "$VERSION"
git push origin "$CURRENT_BRANCH" --tags

# ─── Build ────────────────────────────────────────────────────────────────

# Verify the tag matches the manifest so we can never publish artifacts
# that point at the wrong version.
CONF_VERSION="$(node -e "const c=require('./src-tauri/tauri.conf.json');console.log(c.version)")"
if [[ "$TAG_VERSION" != "$CONF_VERSION" ]]; then
  echo "Version mismatch: tag is $VERSION but tauri.conf.json declares $CONF_VERSION." >&2
  exit 1
fi

# Determine signing key source: caller-provided env var or key file.
SIGNING_KEY_PATH="${TAURI_SIGNING_PRIVATE_KEY_PATH:-$HOME/.tauri/nodepad.key}"

file_mode() {
  if stat -f '%Lp' "$1" >/dev/null 2>&1; then
    stat -f '%Lp' "$1"
  else
    stat -c '%a' "$1"
  fi
}

if [[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  SIGNING_PRIVATE_KEY="$TAURI_SIGNING_PRIVATE_KEY"
elif [[ -f "$SIGNING_KEY_PATH" ]]; then
  KEY_MODE="$(file_mode "$SIGNING_KEY_PATH")"
  case "$KEY_MODE" in
    600|400) ;;
    *)
      chmod 600 "$SIGNING_KEY_PATH"
      ;;
  esac
  SIGNING_PRIVATE_KEY="$(cat "$SIGNING_KEY_PATH")"
else
  echo "Signing key not found at $SIGNING_KEY_PATH; set TAURI_SIGNING_PRIVATE_KEY or TAURI_SIGNING_PRIVATE_KEY_PATH." >&2
  exit 1
fi

# Capture caller-provided password, then unset both caller env vars so they
# don't reach node/gh or other non-signing child processes.
if [[ -n "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]]; then
  SIGNING_PRIVATE_KEY_PASSWORD="$TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
else
  read -rs -p "Signing key password: " SIGNING_PRIVATE_KEY_PASSWORD
  echo
fi
unset TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD

cleanup_signing_secrets() {
  unset SIGNING_PRIVATE_KEY SIGNING_PRIVATE_KEY_PASSWORD
}
trap cleanup_signing_secrets EXIT

echo "→ Building macOS app ($VERSION)..."
TAURI_SIGNING_PRIVATE_KEY="$SIGNING_PRIVATE_KEY" \
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$SIGNING_PRIVATE_KEY_PASSWORD" \
  npm run build
cleanup_signing_secrets
trap - EXIT

# ─── Update manifest ─────────────────────────────────────────────────────

echo "→ Generating update manifest..."
node scripts/generate-latest-json.mjs "$VERSION"

# Verify the generated manifest carries the correct version before publishing.
MANIFEST_VERSION="$(node -e "const m=require('./latest.json');console.log(m.version)")"
if [[ "$MANIFEST_VERSION" != "$TAG_VERSION" ]]; then
  echo "Manifest version mismatch: latest.json says $MANIFEST_VERSION but expected $TAG_VERSION." >&2
  exit 1
fi

# ─── GitHub release ───────────────────────────────────────────────────────

# Collect artifacts
DMG=$(ls src-tauri/target/release/bundle/dmg/*.dmg 2>/dev/null || echo "")
TAR=$(ls src-tauri/target/release/bundle/macos/*.app.tar.gz 2>/dev/null || echo "")
SIG=$(ls src-tauri/target/release/bundle/macos/*.app.tar.gz.sig 2>/dev/null || echo "")

if [[ -z "$DMG" ]]; then
  echo "No DMG found in bundle/dmg/. Build may have failed." >&2
  exit 1
fi

# Extract changelog entry for this version
CHANGELOG_ENTRY=""
if [[ -f "CHANGELOG.md" ]]; then
  CHANGELOG_ENTRY=$(perl -0777 -ne "
    /## \[?\Q$TAG_VERSION\E\]?.*?\n(.*?)\n(?=## \[|\z)/s and print \$1
  " CHANGELOG.md 2>/dev/null || true)
fi

if [[ -z "$CHANGELOG_ENTRY" ]]; then
  CHANGELOG_ENTRY="Release $VERSION"
fi

echo "→ Publishing to GitHub releases..."
gh release create "$VERSION" \
  --repo kiran-brahma/nodepad-app \
  --title "Nodepad $VERSION" \
  --notes "$CHANGELOG_ENTRY" \
  $DMG \
  $TAR \
  $SIG \
  CHANGELOG.md \
  latest.json

echo ""
echo "✓ Release $VERSION published."
echo "  https://github.com/kiran-brahma/nodepad-app/releases/tag/$VERSION"
