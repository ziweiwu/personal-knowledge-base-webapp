# End-to-end tests

Playwright drives the **real binary** serving the **real embedded frontend**. There is no
mock server and no mocked API: what the suite exercises is what ships.

```sh
cd web && npm run e2e          # everything, headless
cd web && npm run e2e:ui       # pick and watch individual tests
cd web && npx playwright test --project=desktop
cd web && npx playwright test --project=phone-landscape
```

## Isolation

`run-server.sh` builds a throwaway world under `$TMPDIR/kbview-e2e` before starting the
server on port 4399:

- every fixture root is **copied** there, so a test that saves, renames, deletes or ticks
  a checkbox can never modify `test/fixtures/`, which is hand-built and verified
  byte-for-byte by `verify_fixtures.py`
- the account and its sessions live there too, so the suite never touches `./data` or the
  real `kbview.config.json`

CI asserts the isolation held by running `git diff --exit-code -- test/fixtures/` after
the suite. If that ever fails, the copies stopped being copies and the run is suspect.

## Environment

Both are read by `run-server.sh`, and both exist so a second suite can run without
colliding with the first — a stale server on 4399 would otherwise be silently reused.

| Variable | Default | Meaning |
|---|---|---|
| `KBVIEW_E2E_PORT` | `4399` | Port the suite's own server listens on |
| `KBVIEW_E2E_DIR` | `$TMPDIR/kbview-e2e` | Scratch directory holding the root copies, the config and the account |

`KBVIEW_E2E_DIR` is wiped and rebuilt on every start, so anything left in it from a
previous run is gone. The Playwright config reads `KBVIEW_E2E_PORT` too, and `helpers.ts`
reads `KBVIEW_E2E_DIR` to check what landed on disk.

## Roots the suite mounts

| Root | Source | For |
|---|---|---|
| `shapes` | `test/e2e/fixtures/content-shapes` | Content shapes, and every write test |
| `vault` | `test/fixtures/obsidian-vault` | Obsidian-mode behaviour |
| `plain` | `test/fixtures/plain-markdown` | A folder with no `.obsidian/` |
| `media` | `test/fixtures/mixed-media` | PDF, DOCX, images, CSV, JSON, opaque binary |

`content-shapes` carries an `.obsidian/` directory, which is the **only** thing that puts
a root into Obsidian mode. Remove it and every `[[wikilink]]` in that corpus silently
becomes literal text — the fixtures still look fine and stop testing anything.

## What the fixtures are for

Nothing in `content-shapes` is decorative. Each file pins a behaviour that has broken:

| File | Pins |
|---|---|
| `frontmatter-callout-tasks.md` | The line-offset corruption: frontmatter and a callout shift parser line numbers away from the file on disk |
| `tasks.md` | Every task shape the scanner and comrak must agree on, including one inside a fence that must never be writable |
| `raw-html-checkbox.md` | A literal `<input type="checkbox">` must make the whole document's boxes read-only rather than pair them off by one |
| `currency.md` | "costs $5 today and $7 tomorrow" must stay prose |
| `math.md` | MathML as one expression, and an `aligned` environment that must degrade in place |
| `code-and-highlighting.md` | Class-based highlighting, a long line that scrolls in its own box, CJK inside a fence that once panicked the renderer |
| `tables.md` | A table wider than any phone |
| `crlf.md` | Line endings a write must preserve exactly |
| `empty.md` | A zero-byte document must reach an explained empty state, never a blank pane |
| `Spaces And Caps.md`, `unicode-标题.md` | Names that have to survive a URL round trip |
| `deep/nested/folder/leaf.md` | Breadcrumbs deep enough to truncate |

## Adding a test

Specs share one server, one account and one corpus, and run with a single worker in file
order. **A test that mutates a document must create that document first** — moving a
fixture out from under a later test is the flake this suite has already produced once.

The session comes from the `setup` project via `storageState`; do not sign in per test.
Login is rate limited and Argon2-expensive, which is exactly what it is for.
