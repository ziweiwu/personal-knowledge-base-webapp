#!/usr/bin/env bash
# Launches the built app against a scratch vault and checks that it really serves.
#
# KBVIEWER_APP_SUPPORT redirects everything the app stores - config, account store and the
# generated credential - so nothing here can touch the real vault setup.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
APP="$REPO/target/macos/KBViewer.app"
PORT=4487
SCRATCH="${TMPDIR:-/tmp}/kbviewer-app-smoke"

if [ ! -d "$APP" ]; then
	echo "smoke-test: $APP is not built — run macos/build-app.sh first" >&2
	exit 1
fi

app_pid=""
foreign_pid=""
stop_pid() {
	[ -n "$1" ] && kill -0 "$1" 2>/dev/null || return 0
	kill -TERM "$1" 2>/dev/null || true
	sleep 2
	kill -KILL "$1" 2>/dev/null || true
}
cleanup() {
	stop_pid "$app_pid"
	stop_pid "$foreign_pid"
	pkill -f "$SCRATCH" 2>/dev/null || true
}
trap cleanup EXIT

fail() { echo "  FAIL: $1" >&2; exit 1; }
pass() { echo "  ok: $1"; }

echo "==> Preparing a scratch vault in $SCRATCH"
rm -rf "$SCRATCH"
mkdir -p "$SCRATCH/vault" "$SCRATCH/support"
printf '# Smoke test vault\n\nA note.\n' > "$SCRATCH/vault/index.md"

cat > "$SCRATCH/support/kbviewer.config.json" <<JSON
{
  "host": "127.0.0.1",
  "port": $PORT,
  "dataDir": "$SCRATCH/support/data",
  "roots": [{ "id": "kb", "name": "Smoke", "path": "$SCRATCH/vault" }]
}
JSON

echo "==> Launching the app"
KBVIEWER_APP_SUPPORT="$SCRATCH/support" "$APP/Contents/MacOS/KBViewer" \
	>"$SCRATCH/app.log" 2>&1 &
app_pid=$!

echo "==> Waiting for the server"
deadline=$((SECONDS + 45))
status=""
while [ $SECONDS -lt $deadline ]; do
	if ! kill -0 "$app_pid" 2>/dev/null; then
		cat "$SCRATCH/app.log" >&2
		fail "the app exited during startup"
	fi
	status="$(curl -s -o "$SCRATCH/probe.json" -w '%{http_code}' \
		"http://127.0.0.1:$PORT/api/auth/session" 2>/dev/null || true)"
	[ "$status" = "401" ] && break
	sleep 1
done

[ "$status" = "401" ] || fail "no JSON 401 from /api/auth/session (got '${status:-nothing}')"
pass "the server answers /api/auth/session with 401"

grep -q '"error":"unauthorized"' "$SCRATCH/probe.json" \
	|| fail "the 401 body is not kbviewer's: $(cat "$SCRATCH/probe.json")"
pass "the 401 body is kbviewer's"

curl -fs "http://127.0.0.1:$PORT/" | grep -qi '<div id="root"' \
	|| fail "the SPA shell was not served at /"
pass "the frontend shell is served"

pgrep -f "kbviewer-server --config $SCRATCH/support/kbviewer.config.json" >/dev/null \
	|| fail "no kbviewer-server child process is running with the scratch config"
pass "the server runs as a child of the app"

# A wrapper that spawns itself instead of the server climbs from one process to
# hundreds within seconds, and every one of them looks idle.
# Counted with wc, not `pgrep -c`: BSD pgrep has no -c, and the usage error it prints
# instead would have made this check pass without ever running.
bundle_processes="$(pgrep -f "$APP/Contents/MacOS" | wc -l | tr -d ' ')"
[ "${bundle_processes:-0}" -ge 2 ] || fail "expected a wrapper and a server, saw $bundle_processes"
[ "$bundle_processes" -le 3 ] \
	|| fail "$bundle_processes processes from the bundle; the wrapper is spawning itself"
pass "the bundle runs one wrapper and one server"

# Proves first-run setup ran the CLI, not just that a server started: without an
# account the server refuses to start at all, so this is what the app had to create.
"$APP/Contents/MacOS/kbviewer-server" --config "$SCRATCH/support/kbviewer.config.json" \
	user list | grep -q "kbviewer-app@localhost" \
	|| fail "first-run setup did not create the app account"
pass "the app account exists"

grep -q "listening on http" "$SCRATCH/support/logs/server.log" \
	|| fail "the server log does not record a listening line"
pass "stderr is captured to the server log"

echo "==> Quitting"
kill -TERM "$app_pid"
deadline=$((SECONDS + 15))
while kill -0 "$app_pid" 2>/dev/null && [ $SECONDS -lt $deadline ]; do sleep 1; done
kill -0 "$app_pid" 2>/dev/null && fail "the app did not quit on SIGTERM"
app_pid=""
pass "the app quits on SIGTERM"

sleep 1
pgrep -f "$SCRATCH/support/kbviewer.config.json" >/dev/null \
	&& fail "the server was left running after the app quit"
pass "no orphaned server process"

curl -s -m 2 -o /dev/null "http://127.0.0.1:$PORT/" \
	&& fail "port $PORT is still answering" || true
pass "the port is closed"

echo
echo "==> A server on the port that does not share the app's account"

# The launch agent in deploy/ is exactly this: a kbviewer on 4321 with the repository's
# config and its own accounts. Adopting it would ignore the folder this app was told to
# serve and strand the user on a login screen the app cannot fill in.
mkdir -p "$SCRATCH/foreign" "$SCRATCH/foreign-vault"
printf '# Foreign vault\n' > "$SCRATCH/foreign-vault/index.md"
cat > "$SCRATCH/foreign/kbviewer.config.json" <<JSON
{
  "host": "127.0.0.1",
  "port": $PORT,
  "dataDir": "$SCRATCH/foreign/data",
  "roots": [{ "id": "kb", "name": "Foreign", "path": "$SCRATCH/foreign-vault" }]
}
JSON
"$APP/Contents/MacOS/kbviewer-server" --config "$SCRATCH/foreign/kbviewer.config.json" \
	user add someone@else.test --password-stdin <<< 'a-long-enough-password' >/dev/null
"$APP/Contents/MacOS/kbviewer-server" --config "$SCRATCH/foreign/kbviewer.config.json" \
	>"$SCRATCH/foreign.log" 2>&1 &
foreign_pid=$!
disown %% 2>/dev/null || true   # otherwise bash prints "Terminated" when it is stopped

deadline=$((SECONDS + 30))
until curl -s -o /dev/null -m 1 "http://127.0.0.1:$PORT/api/auth/session"; do
	[ $SECONDS -lt $deadline ] || fail "the stand-in server never came up"
	sleep 1
done
pass "a foreign server holds port $PORT"

rm -rf "$SCRATCH/support/data" "$SCRATCH/support/logs"
KBVIEWER_APP_SUPPORT="$SCRATCH/support" "$APP/Contents/MacOS/KBViewer" \
	>"$SCRATCH/app-adopt.log" 2>&1 &
app_pid=$!

deadline=$((SECONDS + 60))
until [ -f "$SCRATCH/support/data/sessions.json" ]; do
	[ $SECONDS -lt $deadline ] || fail "the app never signed in to a server of its own"
	sleep 1
done
pass "the app signed in to a server of its own"

grep -q "\"port\" : $PORT," "$SCRATCH/support/kbviewer.config.json" \
	&& fail "the app kept port $PORT and adopted the foreign server"
pass "the app moved to a free port"

pgrep -f "kbviewer-server --config $SCRATCH/support" >/dev/null \
	|| fail "the app did not start a server of its own"
pass "the app runs its own server"

kill -0 "$foreign_pid" 2>/dev/null || fail "the foreign server was killed"
pass "the foreign server was left alone"

stop_pid "$app_pid"; app_pid=""
stop_pid "$foreign_pid"; foreign_pid=""

echo
echo "smoke-test: all checks passed"
