# kbviewer

A web viewer and light editor for folders of documents on your own machine — an Obsidian
vault, a plain folder of markdown, or a pile of PDFs and Word files — reachable from a
phone over Tailscale, behind a password.

- **Markdown** with the Obsidian dialect: wikilinks, embeds and transclusion, callouts,
  tags, backlinks, GFM tables, maths, mermaid diagrams. Task-list checkboxes are live —
  ticking one in the browser writes the single character back to the file.
- **Sized for the device**: a phone screenshot is several megabytes of PNG shown in a
  column a few hundred pixels wide. Images are re-encoded and resized on demand, cached,
  and offered through `srcset` — measured at 21x fewer bytes on a real set of screenshots.
- **Other formats**: `.docx` converted to HTML, PDFs shown inline, images, CSV as a
  sortable table, source files highlighted. Anything else is listed and downloadable.
- **Any folder**, not just vaults. Obsidian features switch on when a `.obsidian/`
  directory is present and stay off when it is not, so a plain folder of markdown behaves
  like a plain folder of markdown.
- **Edit on demand**: view first, edit when you want to, with conflict detection against
  changes made in Obsidian.
- **Live**: edit a note on your desktop and the browser on your phone updates.

## Requirements

Rust stable and Node 22 (what CI uses). Nothing else — no database, no external services.

Running the end-to-end suite additionally needs its browsers, once:

```sh
cd web && npx playwright install chromium webkit
```

Both, not just chromium: the desktop project runs on chromium, and the two phone projects
emulate an iPhone, which Playwright drives with WebKit.

## Quick start

```sh
cp kbviewer.config.example.json kbviewer.config.json   # then set your folder path
cd web && npm install && npm run build && cd ..
cargo build --release
./target/release/kbviewer user add you@example.com
./target/release/kbviewer
```

Then open http://127.0.0.1:4321. For access from other devices, see
[deploy/README.md](deploy/README.md).

## As a Mac app

`KBViewer.app` is a menu bar item and a window of its own. It starts the server itself and
signs you in, so none of the quick start above is needed at run time; it asks for a folder
the first time and remembers it. Details, and what it does when a server is already
running on the port, are in [macos/README.md](macos/README.md).

### Download it — Apple Silicon

Take the `.zip` from
[Releases](https://github.com/ziweiwu/personal-knowledge-base-webapp/releases), unzip it,
and drag `KBViewer.app` into `/Applications`.

**macOS refuses to open it the first time**, saying:

> "KBViewer" is damaged and can't be opened. You should move it to the Trash.

It is not damaged, and the download is not corrupt. The app is signed *ad hoc* rather than
with a paid Apple Developer ID, so Gatekeeper sees something that arrived from the internet
and cannot say who built it — and reports that as damage. To open it anyway: dismiss the
dialog, go to **System Settings → Privacy & Security**, scroll down to the line about
KBViewer, and click **Open Anyway**. Once is enough. The terminal equivalent, if you prefer
it:

```sh
xattr -dr com.apple.quarantine /Applications/KBViewer.app
```

The release is **Apple Silicon only**. The bundle carries a server binary built for the
machine that built it, and no universal build is published — on an Intel Mac, build from
source.

### Build it yourself

```sh
macos/build-app.sh --install
```

Builds the app and copies it to `/Applications`. Needs the Xcode Command Line Tools for
`swiftc`, plus the Node and Rust toolchains the rest of the repo needs. A locally built
copy is never quarantined, so none of the Gatekeeper step above applies to it.

## Container image

CI publishes a multi-architecture image (`linux/amd64` and `linux/arm64`) on every push to
`main`:

```
ghcr.io/ziweiwu/personal-knowledge-base-webapp:latest
```

It is a public package, so pulling needs no credentials. `deploy/compose.yaml` is a
template using it — set your own folder path and run `docker compose up -d`. Create an
account **before** the first start; the server refuses to run with none:

```sh
docker compose run --rm kbviewer user add you@example.com
```

The image is built by cross-compiling on the CI runner and copying the finished binary in,
rather than compiling Rust under emulation, which would take tens of minutes per
architecture. It sets no `USER`: the compose file decides the uid/gid, so one image works
on hosts with different id conventions. **That means without a `user:` line the container
runs as root** — the template sets one.

## Configuration

```json
{
  "host": "127.0.0.1",
  "port": 4321,
  "dataDir": "./data",
  "roots": [
    {
      "id": "kb",
      "name": "Knowledge Base",
      "path": "/absolute/path/to/your/folder",
      "indexNames": ["index.md", "README.md"],
      "wikilinks": null,
      "folderNotes": false,
      "readOnly": false
    }
  ]
}
```

| Field | Meaning |
|---|---|
| `id` | URL-safe identifier; appears in every path |
| `path` | Absolute. **The only folder the server will ever read.** `~` is expanded |
| `indexNames` | Tried in order to find a folder's landing page; a folder with none gets a generated listing |
| `wikilinks` | `null` auto-detects from `.obsidian/`; `true`/`false` forces it |
| `folderNotes` | Treat a note named after its folder as that folder's page |
| `readOnly` | Refuse every write route for this root |

`KBVIEWER_ROOT_<ID>` overrides a root's path (`KBVIEWER_ROOT_KB=/vault`), so one config works
in a container where the folder is mounted elsewhere. `KBVIEWER_CONFIG` moves the config
file itself.

## Security model

Tailscale controls *who can reach the port*. This app controls *who can read the
documents*, which still matters if a tailnet device is compromised or Funnel is ever
switched on.

- **Argon2id** password hashing at OWASP parameters. Login is rate limited per email and
  per address, checked before hashing, with lengthening lockouts.
- **Server-side sessions**, not JWTs, so `kbviewer user revoke` genuinely signs out a lost
  device. `HttpOnly`, `SameSite=Lax`, `Secure` when the connection is HTTPS.
- **No signup route exists.** Accounts come from the CLI only, and the server refuses to
  start with none.
- **Every** `/api` route is behind the gate, file bytes included — an attachment is
  document content.
- **One containment check** (`kbviewer_core::paths::resolve_in_root`) guards every read and
  write. It refuses `..`, absolute paths, and symlinks pointing out of the folder, and it
  bounds paths that do not exist yet. `.obsidian/`, `.trash/`, `.git/` and Synology's
  `@eaDir` are never served.
- Writes are atomic, and delete moves to `.trash/` rather than destroying anything.

**Accepted risk:** raw HTML inside your documents is rendered, as Obsidian does. The
content is yours and sits behind auth, so this is self-inflicted-only. Serving a folder
you do not trust would need sanitising added.

## Not supported

Stated plainly so nothing looks broken when it is merely absent:

- `.doc` (the pre-2007 binary format), `.xlsx`, `.pptx` — listed and downloadable, not rendered.
- PDF **text** is not indexed for search. PDFs are viewable; their contents are not searchable.
- LaTeX environments beyond what `latex2mathml` supports — `aligned` among them — show
  their source with a visible warning instead of rendering.
- Inline maths may not open with a digit: `$5x$` renders literally. This is deliberate.
  A knowledge base is full of currency, and a permissive `$…$` scanner pairs "costs $5"
  with a later `$`, silently swallowing the prose between them. Write `$x5$` or use a
  display block. Currency is always safe.
- `.docx` gaps are documented in `crates/kbviewer-docx`: style inheritance, `rowspan` from
  `vMerge`, list numbering overrides, text boxes, footnotes and headers.

## Layout

```
crates/kbviewer-core/   documents, paths, links, index, search, wire types
crates/kbviewer-docx/   OOXML to HTML, isolated so it can be tested hard
crates/kbviewer-server/ axum, auth, rendering, watching
web/                  Vite + React + TypeScript
test/fixtures/        three folders exercising vault / plain / mixed-media behaviour
test/e2e/             Playwright fixtures and the isolated server the suite runs against
web/e2e/              the end-to-end specs themselves
deploy/               Dockerfiles, compose template, macOS launch agent
macos/                the KBViewer.app wrapper: Swift sources, bundle build, smoke test
.github/workflows/    CI: tests, then publishes the container image
.cargo/config.toml    points ts-rs at web/src/api so the bindings stay generated
```

## Development

```sh
cargo test --workspace                       # 321 tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cd web && npm run dev                        # proxies /api to 127.0.0.1:4321
cd web && npm run typecheck                  # tsc --noEmit
cd web && npm run lint                       # eslint
cd web && npm run e2e                        # Playwright, against the real binary
python3 test/fixtures/verify_fixtures.py     # checks the binary fixtures are still valid
```

The end-to-end suite drives the actual binary serving the actual embedded frontend — no
mock server, no mocked API — across a desktop viewport and a phone in both orientations.
It runs against **copies** of the fixture roots in a scratch directory, so a test that
saves, renames or deletes can never modify the fixtures themselves. `test/e2e/README.md`
explains the corpus and what each file is there to pin.

CI runs exactly this list, and additionally fails if the generated API bindings differ
from what is committed, or if the e2e run left a fingerprint on `test/fixtures/`.

`web/src/api/types.ts` is **generated** from the Rust types by `cargo test` — never edit
it by hand. If the API changes, rerun the tests and commit the regenerated file.
