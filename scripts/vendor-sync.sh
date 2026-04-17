#!/bin/bash
set -euo pipefail

# ============================================
# Configuration (edit or set as env vars)
# ============================================
UPSTREAM_URL="${UPSTREAM_URL:-https://github.com/modrinth/code}"
UPSTREAM_FOLDER="${UPSTREAM_FOLDER:-apps/app}"
LOCAL_FOLDER="${LOCAL_FOLDER:-apps/app/tauri}"
REMOTE_NAME="${REMOTE_NAME:-vendor-upstream}"

# ============================================
# Usage
# ============================================
usage() {
  cat <<EOF
Usage: $0 [ref]

Vendors a subfolder from an external Git repo into this repo.

Arguments:
  ref   Git ref to fetch (tag, branch, commit). Defaults to 'main'.

Required config (set as env vars or edit script):
  UPSTREAM_URL     External repo URL
  UPSTREAM_FOLDER  Subfolder path in upstream repo (e.g. packages/core)
  LOCAL_FOLDER     Destination folder in your repo (e.g. apps/tauri/vendor)
  REMOTE_NAME      Git remote name (default: vendor-upstream)

Examples:
  UPSTREAM_URL=https://github.com/user/repo UPSTREAM_FOLDER=packages/core LOCAL_FOLDER=vendor $0 v1.2.3
  $0 main
EOF
  exit 1
}

# ============================================
# Validate
# ============================================
if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
fi

if [[ -z "$UPSTREAM_URL" || -z "$UPSTREAM_FOLDER" || -z "$LOCAL_FOLDER" ]]; then
  echo "Error: Missing required configuration" >&2
  usage
fi

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "Error: Not inside a git repository" >&2
  exit 1
fi

# ============================================
# Setup
# ============================================
REF="${1:-main}"
REPO_ROOT="$(git rev-parse --show-toplevel)"
LOCAL_PATH="$REPO_ROOT/$LOCAL_FOLDER"

# Calculate strip-components count
IFS='/' read -ra PATH_PARTS <<< "$UPSTREAM_FOLDER"
STRIP_COUNT=${#PATH_PARTS[@]}

# ============================================
# Step 1: Register remote (idempotent)
# ============================================
echo ":: Registering remote '$REMOTE_NAME'..."
if git remote | grep -q "^${REMOTE_NAME}$"; then
  echo "   Remote '$REMOTE_NAME' already exists"
else
  git remote add "$REMOTE_NAME" "$UPSTREAM_URL"
  echo "   Added remote: $REMOTE_NAME -> $UPSTREAM_URL"
fi

# ============================================
# Step 2: Shallow fetch
# ============================================
echo ":: Fetching ref '$REF' (shallow)..."
git fetch "$REMOTE_NAME" "$REF" --depth=1 --no-tags 2>/dev/null || {
  echo "   Ref '$REF' not found as branch, trying as tag..."
  git fetch "$REMOTE_NAME" "refs/tags/$REF" --depth=1 --no-tags
}

COMMIT_HASH="$(git rev-parse FETCH_HEAD)"
TIMESTAMP="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo "   Resolved to: $COMMIT_HASH"

# ============================================
# Step 3: Extract subfolder
# ============================================
echo ":: Extracting '$UPSTREAM_FOLDER' -> '$LOCAL_FOLDER'..."
rm -rf "$LOCAL_PATH"
mkdir -p "$LOCAL_PATH"
git archive FETCH_HEAD "$UPSTREAM_FOLDER" | tar -x -C "$LOCAL_PATH" --strip-components="$STRIP_COUNT"

# ============================================
# Step 4: Write VENDOR.md
# ============================================
echo ":: Writing VENDOR.md..."
cat > "$LOCAL_PATH/VENDOR.md" <<EOF
# Vendored from Upstream

- **Upstream URL:** $UPSTREAM_URL
- **Upstream Folder:** \`$UPSTREAM_FOLDER\`
- **Ref:** \`$REF\`
- **Commit:** \`$COMMIT_HASH\`
- **Timestamp:** $TIMESTAMP
EOF

# ============================================
# Step 5: Commit (if changed)
# ============================================
echo ":: Checking for changes..."
cd "$REPO_ROOT"
if git diff --quiet "$LOCAL_PATH" && git diff --cached --quiet "$LOCAL_PATH"; then
  echo "   No changes, skipping commit"
else
  git add "$LOCAL_PATH"
  git commit -m "chore(vendor): sync $UPSTREAM_FOLDER from $UPSTREAM_URL @ $REF"
  echo "   Committed changes"
fi

echo ":: Done"