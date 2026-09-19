# QA recipe — `personal-knowledge-base-webapp` (kbviewer)

Read by `qa-bar-raiser` and `ux-bar-raiser` before they touch this app. Every
path, port and script name below is specific to this repo; pointed at any other
they are a concrete recipe for the wrong app.

Values marked `GUESSED` were not verified; everything else was read from the
file named beside it.

## Setup

```
cd ~/Projects/personal-knowledge-base-webapp
export PATH="$HOME/.cargo/bin:$PATH"            # run-server.sh does the same
(cd web && npm ci && npm run build)             # only when web/src changed; cargo embeds web/dist
KBVIEWER_E2E_PORT=4500 \
KBVIEWER_E2E_DIR="$TMPDIR/kbviewer-qa" \
  bash test/e2e/run-server.sh &                 # QA scratch server, fixture mode
```

`test/e2e/run-server.sh` wipes and rebuilds `KBVIEWER_E2E_DIR`, **copies** four
fixture roots into it, writes its own config there, builds
`target/debug/kbviewer` (`cargo build -p kbviewer-server`), creates one account,
and execs the binary bound to `127.0.0.1:$KBVIEWER_E2E_PORT`. It refuses to start
without `web/dist/index.html`. Nothing it does touches the repo's `./data`,
`kbviewer.config.json`, `kbview.config.json` or `test/fixtures/`.

Account for the scratch server (from `run-server.sh`, not a secret):
email `e2e@example.test`, password `e2e-password-not-a-secret`.

**Live data — never point anything at these:**

- **Port 4321** on this Mac: `kbview.config.json` at the repo root maps root
  `kb` to `~/SynologyDrive/my-knowledge-base`, the user's real vault, and
  `./data` holds the real account and sessions. A running server there is the
  user's, not yours.
- **`my-nas:4321`** (Synology container `kbview`): the same vault, synced.
  Writes there reach every device within seconds.
- **`test/fixtures/` and `test/e2e/fixtures/`** in the checkout: hand-built,
  verified byte for byte by `python3 test/fixtures/verify_fixtures.py`, and CI
  fails on `git diff --exit-code -- test/fixtures/`. Only ever drive the copies
  under `KBVIEWER_E2E_DIR`.
- **Port 4399** is the e2e suite's own server; leave it for `npm run e2e`.

`ux-bar-raiser` uses **4400** and `qa-bar-raiser` uses **4500**, each with its
own `KBVIEWER_E2E_DIR` (`$TMPDIR/kbviewer-ux`, `$TMPDIR/kbviewer-qa`), so both
can run without fighting over one server. Both still serialise on the browser:
two agents driving Chrome MCP share one real Chrome.

**Mock mode:** two exist, pick by what you are testing.

- *Fixture mode* (above) is the real binary and the real embedded UI over copied
  fixtures. Use it for everything that touches the API or disk.
- *Frontend-only mock:* `cd web && npm run dev:mock` (`VITE_MOCK=1`, transport
  from `web/src/api/mock.ts`). No server, no disk; useful only for layout work.
  Its port is 5173 (`web/vite.config.ts`) and it does not exercise INV-1
  through INV-15 at all.

## Fixtures

Four roots are mounted (`test/e2e/README.md`):

| Root | Source | Mode |
|---|---|---|
| `shapes` | `test/e2e/fixtures/content-shapes` | Obsidian (has `.obsidian/`) — every write test lives here |
| `vault` | `test/fixtures/obsidian-vault` | Obsidian; nested folders, CJK filename, PDF/SVG/PNG attachments |
| `plain` | `test/fixtures/plain-markdown` | **No** `.obsidian/`; relative markdown links, not wikilinks |
| `media` | `test/fixtures/mixed-media` | PDF, DOCX, JPG, PNG, SVG, CSV, JSON, LICENSE (no extension), `opaque.bin` |

`content-shapes` is deliberately hostile and every file pins something that
broke once: `frontmatter-callout-tasks.md` (line offsets), `tasks.md` (a task
inside a fence that must never be writable), `raw-html-checkbox.md` (a literal
`<input>` that must disable every checkbox), `currency.md` (`$5 ... $7` is not
maths), `math.md` (`aligned` must degrade in place), `crlf.md`,
`no-trailing-newline.md`, `empty.md` (zero bytes), `Spaces And Caps.md`,
`unicode-标题.md`, `deep/nested/folder/leaf.md`, `tables.md` (wider than a
phone), `images.md` (1600 px, 2000 px tall, 4 px), `long-document.md`,
`far-below-fold.md`, `mermaid.md`, `callouts.md`, `links.md`, `notes.txt`,
`data/inventory.csv`, `data/settings.json`.

What the fixtures are **flattering** about, so you have to bring it yourself:

- No `readOnly` root is mounted. Add one to a copy of the generated
  `kbviewer.config.json` in `KBVIEWER_E2E_DIR` and restart if you need INV-13.
- No symlink exists in any fixture (git would carry it, verify_fixtures.py
  would reject it). Create one inside `KBVIEWER_E2E_DIR/roots/shapes` pointing
  outside for INV-1.
- Nothing is concurrent. Two clients on one path is entirely on you (C-2, C-3).
- No file is owned by another uid. C-1 can only be measured on the NAS.
- No `@eaDir`, `.SynologyWorkingDirectory` or `.trash` content ships; make them.
- The DOCX is well-formed. A zip bomb or a docx missing `word/document.xml`
  must be constructed (see `verify_fixtures.py` for the parts it requires).

Rules from `test/e2e/README.md` that apply to any driver, not just Playwright:
a test that mutates a document creates that document first; do not sign in per
action (login is rate limited and Argon2-expensive); the phone projects are
WebKit, so a Chrome-only pass says nothing about the phone.

## The gates, in the order that pays

**1. Read the source against `INVARIANTS.md` first.** Start at the Candidates
section; each names the one check that settles it. Then INV-1 to INV-7 in
order, which is severity order. The files that own them:
`crates/kbviewer-core/src/paths.rs`, `crates/kbviewer-server/src/routes/write.rs`,
`crates/kbviewer-core/src/tasks.rs`, `crates/kbviewer-core/src/links.rs`,
`crates/kbviewer-server/src/auth/middleware.rs`, `web/src/api/client.ts`.

**2. Run the audit scripts this repo already ships.** They are what CI runs
(`.github/workflows/ci.yml`), in this order:

```
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace 2>&1 | grep -E 'FAILED|panicked|error(\[|:)|test result' -A5
git diff --exit-code -- web/src/api/types.ts      # ts-rs bindings drifted from the Rust types
(cd web && npx tsc --noEmit && npm run lint)
python3 test/fixtures/verify_fixtures.py          # fixtures are what the tests assume
(cd web && npm run e2e 2>&1 | grep -E '✘|failed|Error' -A5)   # own server on 4399; needs chromium+webkit installed
git diff --exit-code -- test/fixtures/            # the e2e run modified nothing it shouldn't
```

`npm run e2e` takes the port from `KBVIEWER_E2E_PORT` too; if 4399 is busy the
suite silently reuses whatever answers there, so check the port is free first.
The Playwright HTML report lands in `web/playwright-report/` (Playwright's
default; `web/playwright.config.ts` adds the `html` reporter only when `CI` is
set, locally it is the list reporter). Screenshots from a failed run land in
`web/test-results/` (Playwright's default; both directories are in `.gitignore`).

**3. Fuzz last, as a regression net.** There is no fuzz harness in the repo.
Useful targets if you write a capped one: random paths at
`/api/doc/shapes/<p>` (INV-1, INV-10), random `baseMtimeMs` (INV-2), random
`line` on `/api/task` (INV-3), random bytes to `/api/file/shapes/<name>`
(INV-16). Seed it and name the seed in the report. When it comes back clean,
say which oracle from "What has no oracle" you would add rather than adding
seeds.

## Known non-issues

- **Raw HTML in a note renders as HTML.** `render.unsafe = true` in
  `render/markdown.rs` is deliberate and documented in the README as accepted
  risk; the content is the authenticated user's own vault. Not a finding.
- **An account added by the CLI while the server runs does not exist until
  restart.** `AuthStore` reads accounts once at startup; documented in
  `macos/README.md`.
- **`IN_OPEN`/`IN_CLOSE_NOWRITE` from inotify.** Filtered by `watch.rs::is_change`
  since 2026-09-19; a reindex loop shows as repeated `reindexing after change`
  lines and 500 MiB of RSS. Measured 7 MiB idle after the fix.
- **The NAS vault has no `.obsidian/`.** Synology Drive skips dot-folders; the
  NAS config carries `"wikilinks": true`. A fixture root without `.obsidian/`
  rendering `[[links]]` as text is correct behaviour for a plain root.
- **`/api/file` returns 401 with no body to an unauthenticated request.**
  Asserted by `file_bytes_are_not_public`; not a broken image.
- **A 2 MiB note saving fine.** axum's default body limit is raised per route
  in `router.rs` (16 MiB for documents, 64 MiB for uploads).
- **A toggle on an already-ticked box returns 200 and writes nothing.**
  `TaskError::AlreadySet` is success by design (two clicks racing).
- **Renaming `note.md` to `Note.md` succeeds on the Mac.** Case-only rename on
  a case-insensitive filesystem is allowed on purpose
  (`reject_occupied_destination`). On the NAS (ext4/btrfs) both can exist.
