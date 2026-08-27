# kbview

A web viewer and light editor for folders of documents on your own machine — an Obsidian
vault, a plain folder of markdown, or a pile of PDFs and Word files — reachable from a
phone over Tailscale, behind a password.

- **Markdown** with the Obsidian dialect: wikilinks, embeds and transclusion, callouts,
  tags, backlinks, GFM tables and task lists, maths, mermaid diagrams.
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

## Quick start

```sh
cp kbview.config.example.json kbview.config.json   # then set your folder path
cd web && npm install && npm run build && cd ..
cargo build --release
./target/release/kbview user add you@example.com
./target/release/kbview
```

Then open http://127.0.0.1:4321. For access from other devices, see
[deploy/README.md](deploy/README.md).

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
docker compose run --rm kbview user add you@example.com
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

`KBVIEW_ROOT_<ID>` overrides a root's path (`KBVIEW_ROOT_KB=/vault`), so one config works
in a container where the folder is mounted elsewhere. `KBVIEW_CONFIG` moves the config
file itself.

## Security model

Tailscale controls *who can reach the port*. This app controls *who can read the
documents*, which still matters if a tailnet device is compromised or Funnel is ever
switched on.

- **Argon2id** password hashing at OWASP parameters. Login is rate limited per email and
  per address, checked before hashing, with lengthening lockouts.
- **Server-side sessions**, not JWTs, so `kbview user revoke` genuinely signs out a lost
  device. `HttpOnly`, `SameSite=Lax`, `Secure` when the connection is HTTPS.
- **No signup route exists.** Accounts come from the CLI only, and the server refuses to
  start with none.
- **Every** `/api` route is behind the gate, file bytes included — an attachment is
  document content.
- **One containment check** (`kbview_core::paths::resolve_in_root`) guards every read and
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
- `.docx` gaps are documented in `crates/kbview-docx`: style inheritance, `rowspan` from
  `vMerge`, list numbering overrides, text boxes, footnotes and headers.

## Layout

```
crates/kbview-core/   documents, paths, links, index, search, wire types
crates/kbview-docx/   OOXML to HTML, isolated so it can be tested hard
crates/kbview-server/ axum, auth, rendering, watching
web/                  Vite + React + TypeScript
test/fixtures/        three folders exercising vault / plain / mixed-media behaviour
deploy/               Dockerfiles, compose template, macOS launch agent
.github/workflows/    CI: tests, then publishes the container image
.cargo/config.toml    points ts-rs at web/src/api so the bindings stay generated
```

## Development

```sh
cargo test --workspace                       # 278 tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cd web && npm run dev                        # proxies /api to 127.0.0.1:4321
cd web && npm run typecheck                  # tsc --noEmit
cd web && npm run lint                       # eslint
python3 test/fixtures/verify_fixtures.py     # checks the binary fixtures are still valid
```

CI runs exactly this list, and additionally fails if the generated API bindings differ
from what is committed.

`web/src/api/types.ts` is **generated** from the Rust types by `cargo test` — never edit
it by hand. If the API changes, rerun the tests and commit the regenerated file.
