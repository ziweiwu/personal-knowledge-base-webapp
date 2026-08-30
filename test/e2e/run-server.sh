#!/usr/bin/env bash
#
# Boot a kbviewer instance the e2e suite owns outright.
#
# Everything it touches lives in a scratch directory: the roots are *copies*, so a test
# that saves, renames, deletes or ticks a checkbox can never dirty the repo's fixtures —
# which are hand-built, verified by verify_fixtures.py, and must stay byte-identical.
# The account and its sessions live there too, so the suite never sees, or writes to, the
# real ./data or the real kbviewer.config.json.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PORT="${KBVIEWER_E2E_PORT:-4399}"
WORK="${KBVIEWER_E2E_DIR:-${TMPDIR:-/tmp}/kbviewer-e2e}"
EMAIL="e2e@example.test"
PASSWORD="e2e-password-not-a-secret"

export PATH="$HOME/.cargo/bin:$PATH"

rm -rf "$WORK"
mkdir -p "$WORK/roots" "$WORK/data"

# Copies, never the originals.
cp -R "$REPO/test/e2e/fixtures/content-shapes" "$WORK/roots/content-shapes"
cp -R "$REPO/test/fixtures/obsidian-vault"     "$WORK/roots/obsidian-vault"
cp -R "$REPO/test/fixtures/plain-markdown"     "$WORK/roots/plain-markdown"
cp -R "$REPO/test/fixtures/mixed-media"        "$WORK/roots/mixed-media"

cat > "$WORK/kbviewer.config.json" <<JSON
{
  "host": "127.0.0.1",
  "port": $PORT,
  "dataDir": "$WORK/data",
  "roots": [
    { "id": "shapes", "name": "Content Shapes",   "path": "$WORK/roots/content-shapes" },
    { "id": "vault",  "name": "Obsidian Vault",   "path": "$WORK/roots/obsidian-vault" },
    { "id": "plain",  "name": "Plain Markdown",   "path": "$WORK/roots/plain-markdown" },
    { "id": "media",  "name": "Mixed Media",      "path": "$WORK/roots/mixed-media" }
  ]
}
JSON

export KBVIEWER_CONFIG="$WORK/kbviewer.config.json"

# The frontend is embedded from web/dist, so it has to exist before the binary is built.
if [ ! -f "$REPO/web/dist/index.html" ]; then
  echo "e2e: web/dist is missing — run 'npm run build' in web/ first" >&2
  exit 1
fi

cargo build --quiet -p kbviewer-server
BIN="$REPO/target/debug/kbviewer"

# The server refuses to start with no accounts, which is the point of it.
"$BIN" user add "$EMAIL" --password-stdin <<< "$PASSWORD" >/dev/null

exec "$BIN"
