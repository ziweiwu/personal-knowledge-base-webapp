# Deploying kbview

The server binds `127.0.0.1` and never terminates TLS itself. Exposure is Tailscale's
job, which means there is no certificate to manage and nothing listening on a public
interface even by accident.

## Exposing it on your tailnet

The Tailscale CLI on macOS lives inside the app bundle and is **not** on `PATH`:

```sh
TS=/Applications/Tailscale.app/Contents/MacOS/Tailscale
"$TS" serve --bg 4321
"$TS" serve status
```

That publishes `https://<machine>.<tailnet>.ts.net` with a real certificate, terminating
TLS and forwarding plain HTTP to the local port. The app reads `X-Forwarded-Proto` from it
and marks the session cookie `Secure` automatically.

Take it off the tailnet again with:

```sh
"$TS" serve --https=443 off
```

`serve` is tailnet-only. Do **not** use `funnel` unless you specifically intend to publish
your documents to the public internet.

## macOS, as a launch agent

`deploy/com.kbview.plist` is a template. Substituting it into place leaves the
template itself untouched, so the same command works on the next machine:

```sh
cargo build --release
mkdir -p ~/Library/LaunchAgents
sed "s|PROJECT_DIR|$PWD|g" deploy/com.kbview.plist > ~/Library/LaunchAgents/com.kbview.plist
launchctl load ~/Library/LaunchAgents/com.kbview.plist
```

Confirm it came up, and that the auth gate is in front of it:

```sh
launchctl list | grep com.kbview                                        # pid, then 0
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:4321/api/roots  # 401
```

`KeepAlive` is on, so killing the process restarts it; stopping it for real means
`launchctl unload`. Logs go to `data/kbview.log`.

```sh
launchctl unload ~/Library/LaunchAgents/com.kbview.plist   # stop
launchctl load   ~/Library/LaunchAgents/com.kbview.plist   # start
rm ~/Library/LaunchAgents/com.kbview.plist                 # uninstall, after unload
```

Rebuild the binary and the agent keeps running the old one until you
`unload` and `load` again; the frontend is embedded in the binary, so a `web`
change needs the frontend built *before* `cargo build --release`.

The Mac must be awake to serve. If you want the knowledge base reachable while it sleeps,
run it on the NAS instead.

### The launch agent and KBView.app

Both want port 4321, and nothing breaks if both are set up. `KBView.app` probes the port
before binding and asks whether the server there accepts its own account. The agent's
does not — it runs this repository's config against `data/` here, not the app's account
store — so the app declines to adopt it, takes the next free port, and runs its own
server alongside. Adopting it would have served the agent's folders instead of the one
the app was told to serve, and left you on a login screen the app cannot fill in.

So by default the two coexist, each with its own port and its own accounts. Giving the
app a config that names this same `data/` directory makes it adopt the agent's server
instead, which is usually what you want with Tailscale in the picture —
`../macos/README.md` has the steps, including the agent restart that `AuthStore` makes
necessary.

Either way, If you use the app, the
agent is largely redundant: the app starts a server when you open it. The agent still
earns its place if you want the server up while the app is closed — serving your phone
over Tailscale with nothing open on the Mac. See [../macos/README.md](../macos/README.md).

## Docker

`deploy/compose.yaml` is a **template, not a deployment**. Every path in it that
names your documents is a placeholder; edit them before running anything. It has no
`build:` section on purpose — building this workspace compiles a Rust binary and a
Vite frontend, which is slow and memory-hungry on the kind of machine this usually
lands on, and a `build:` key makes every `up -d` pay that cost.

The image is a public package published by this repo's CI:

```
ghcr.io/ziweiwu/personal-knowledge-base-webapp:latest
```

Public means no `docker login` is needed to pull it. To build it yourself instead,
use `deploy/Dockerfile` directly and tag the result as that image name. Building on
an arm64 Mac for an x86 host needs `docker buildx build --platform linux/amd64`.

### First start, in order

Step 3 must come before step 4. The server refuses to start with zero accounts, so
`up -d` first just produces a crash loop.

```sh
# 1. Config. "host" must be 0.0.0.0 (a container's loopback is its own, so the
#    127.0.0.1 default would publish nothing), "dataDir" must be /data, and the
#    root's "id" must be "kb" to match KBVIEW_ROOT_KB in the compose file.
cp kbview.config.example.json deploy/kbview.config.json
$EDITOR deploy/kbview.config.json

# 2. Point the vault mount at your folder, and set `user:` to the UID:GID that
#    owns it. Both are marked in the file.
$EDITOR deploy/compose.yaml

docker compose -f deploy/compose.yaml pull

# 3. Create the first account. Interactive, prompts for a password, echo off.
#    `run --rm` shares the service's volumes, so this writes deploy/data/users.json.
#    It does not publish port 4321, so it cannot collide with a running instance.
docker compose -f deploy/compose.yaml run --rm kbview user add you@example.com

# 4. Start it.
docker compose -f deploy/compose.yaml up -d
docker compose -f deploy/compose.yaml logs --tail 50
```

Non-interactive account creation, for scripted provisioning:

```sh
printf '%s' 'the-password' | docker compose -f deploy/compose.yaml \
  run --rm -T kbview user add you@example.com --password-stdin
```

Updating is `docker compose -f deploy/compose.yaml pull` then `up -d`. If you run
Watchtower or similar, note that a `:latest` tag from a registry **will** be rolled
forward unattended; pin a version tag if you do not want that.

### Two things to get right before the first run

- **The folder path.** If the path you mount at `/vault` does not exist, Docker
  creates an empty directory and kbview starts normally on an empty folder. There
  is no error to notice. Check the path exists — and, for a vault, that it contains
  `.obsidian` — before starting, not after.
- **The UID:GID.** The image sets no `USER` — that belongs to the deployment, not
  the image — so a compose file with no `user:` key runs the container as **root**
  and writes root-owned files into your documents folder. Set `user:` to
  `id -u`:`id -g` for the account that owns that folder. Setting it merely *wrong*
  is quieter and nastier than leaving it out: reads work, and only saving an edit
  fails, which looks like a bug in the editor and is not one.

### The maintainer's own deployment is a different file

This repo's `deploy/compose.yaml` is generic on purpose. The Synology NAS it
actually runs on has its own copy, kept outside this repo, carrying that host's
real vault path, its user/group convention, its shared bridge network, its
dashboard labels and its log rotation. The two files are **not** meant to be
identical and neither should be edited to match the other.

## Accounts

There is no signup page. Accounts exist only if someone with shell access creates one, and
the server refuses to start with none rather than presenting a login it cannot satisfy.

```sh
kbview user add you@example.com          # prompts, echo off
kbview user add you@example.com --password-stdin < secret.txt
kbview user list
kbview user passwd you@example.com
kbview user revoke                        # sign out every device, e.g. a lost phone
```

`data/users.json` holds Argon2id hashes and is written `0600`. Back up `data/` — losing it
loses your accounts, though not a single document.
