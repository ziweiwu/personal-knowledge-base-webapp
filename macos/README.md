# KBViewer.app

A macOS wrapper around the `kbviewer` server: a menu bar item, a dedicated window, and
no terminal. It changes nothing about the server — it drives the same CLI and the same
HTTP API that a person would.

## Build

```sh
macos/build-app.sh              # -> target/macos/KBViewer.app
macos/build-app.sh --install    # also copies it to /Applications
```

Needs the Xcode Command Line Tools for `swiftc`, plus the Node and Rust toolchains the
rest of the repo needs. The script builds the frontend first because the server embeds
`web/dist` with `rust-embed`, and the derive fails outright if that folder is missing.

## What it does on first launch

1. Asks for the folder to serve, and writes
   `~/Library/Application Support/KBViewer/kbviewer.config.json`.
2. Creates an account, `kbviewer-app@localhost`, by running the bundled
   `kbviewer user add --password-stdin` — the same path a person would use, since the
   server has no signup route.
3. Saves the generated password to `app-credentials.json` beside `users.json` in the
   configured `dataDir`, mode 0600 - next to the account it opens, so re-pointing the
   config at another data directory moves both together.

Afterwards it signs in for you: the session cookie is injected into the web view before
the page loads, so the app opens straight into the vault. Sessions last 30 days, so most
launches do not sign in at all.

The server's own authentication is untouched — the app just holds the password so you do
not have to. That matters because `tailscale serve` publishes port 4321 to your whole
tailnet; anything reachable there still requires a login.

| Path | Holds |
|---|---|
| `~/Library/Application Support/KBViewer/kbviewer.config.json` | the config |
| the config's `dataDir` (default `…/KBViewer/data/`) | `users.json`, `sessions.json`, `app-credentials.json` |
| `~/Library/Logs/KBViewer/server.log` | the server's stderr |

## Sharing the port

The LaunchAgent in `deploy/` and a manual `./target/release/kbviewer` both want 4321, so
the app probes before it binds:

- a kbviewer that **accepts the app's account** → it adopts that server rather than
  starting a second one, and reuses the session that check established. "Restart Server"
  is disabled, because it is not the app's to restart;
- a kbviewer that does not know the account — the launch agent, which serves the
  repository's config from its own account store — → **not** adopted. Adopting it would
  ignore the folder the app was told to serve and strand you on a login screen the app
  cannot fill in, so the app takes the next free port and runs its own alongside it;
- something else on the port → same: next free port, recorded in the app's own config;
- nothing there → it starts its own, and stops it again on quit.

Sharing an account is the usable test for "the same server": there is no other way to ask
a running kbviewer which configuration it was given.

## Sharing one server with the launch agent

By default the app runs its own server, so with the launch agent also running you get two
servers and two vaults. Pointing both at the same configuration collapses that to one.

Copy the agent's config into the app's, with `dataDir` made absolute - the agent gets
`./data` relative to its `WorkingDirectory`, which a launched app does not have - then
create the app's account in that shared store and restart the agent:

    REPO=/path/to/this/repo
    SUPPORT=~/Library/Application\ Support/KBViewer

    "$REPO/target/release/kbviewer" --config "$SUPPORT/kbviewer.config.json" \
        user add kbviewer-app@localhost
    launchctl kickstart -k "gui/$(id -u)/com.kbviewer"

**The restart is not optional.** `AuthStore` reads `users.json` once at startup and holds
it in memory, so a running server never sees an account the CLI just added - and the app,
finding its account rejected, declines to adopt and starts a second server instead.

The app then adopts the agent's server on 4321: one server, one index, and whatever
`tailscale serve` publishes is the same vault the app shows. To undo it, delete the app's
`kbviewer.config.json` and it will ask for a folder again on the next launch.

Two things to leave alone in that arrangement:

- **Keep the launch agent.** Tailscale is pinned to 4321. If the agent goes away while the
  app is closed, nothing is on that port.
- **Drop roots that do not exist.** A missing folder puts a warning in front of the app on
  every launch; the repo config's `/tmp/kbviewer-scratch` is one such.

## Testing

```sh
macos/smoke-test.sh
```

Launches the built bundle against a scratch vault, checks that it serves and that it
leaves no orphaned server behind. `KBVIEWER_APP_SUPPORT` redirects the config, the account
store and the generated credential, so a test run cannot touch the real setup. That
variable exists for this script; nothing else sets it.

## Handing a build to someone else

```sh
macos/build-app.sh --zip
gh release create v0.1.0 --generate-notes target/macos/KBViewer-0.1.0-arm64.zip
```

`--zip` packs the signed bundle with `ditto` into `target/macos/KBViewer-<version>-<arch>.zip`.
`ditto` preserves symlinks and extended attributes and is the form Apple's notarisation
flow takes; `zip -r` preserves neither. The bundle is flat enough today that both round
trip with the signature intact — checked, not assumed — but that stops being true the
moment it gains a framework.

Refused it will be, because the signature is ad hoc: whoever downloads it is told the app
is **damaged** and has to allow it once through System Settings → Privacy & Security. The
root README states that above the download link, which is the entire trick — the message
names the wrong cause, so anyone who meets it unwarned concludes the download is corrupt
and gives up. Signing with a paid Developer ID and notarising removes the step altogether
(`codesign --options runtime --timestamp --sign "$DEV_ID"` on both binaries, then
`notarytool submit` and `stapler staple`); that is the only thing buying one gets you.

The architecture is in the archive's name because the build is not universal — see below.

## Known rough edges

- **Ad-hoc signing changes the app's identity on every build.** macOS may re-ask for
  access to the vault folder after each `build-app.sh`. Choosing "Always Allow" holds
  until the next rebuild. A stable self-signed certificate would fix it permanently.
  (This is also why the password is not in the login keychain: each rebuild would look
  like a different app asking for someone else's secret, and prompt.)
- **Auto-login means an unlocked Mac is an open vault.** Sign out from inside the app to
  get the login screen back.
- The bundled server is built for this machine's architecture only.
- Nobody knows the generated password, so "Copy Sign-in Details" in the menu puts the
  email and password on the clipboard for signing in from a browser or a phone.

## The one thing not to change

The server inside the bundle is `Contents/MacOS/kbviewer-server`, **not** `kbviewer`. The
wrapper's own executable is `KBViewer`, and the Mac's filesystem is case insensitive, so
`kbviewer` is the same file as `KBViewer`: copying the server in under that name overwrites
one binary with the other and leaves a single working file with no error. Whichever
survived, the result was wrong — the server ran against whatever config the working
directory happened to hold, or the wrapper spawned itself once per generation until the
process table filled. `build-app.sh` asserts the two files are distinct.
