# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Toolchain

Rust is installed via Homebrew's rustup and is **not on `PATH`**. Every command needs:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
```

`ls` produces no output in some sandboxes here — use `find`, `stat` or `cat` instead.

## Commands

```sh
cargo test --workspace                                  # everything
cargo test -p kbview-core links                          # one module
cargo test -p kbview-server --test api                   # HTTP integration tests
cargo test -p kbview-server -- --nocapture rename         # one test, with output
cargo clippy --workspace --all-targets -- -D warnings     # must be clean
cargo fmt --all

cd web && npm run dev        # Vite, proxies /api to 127.0.0.1:4321
cd web && npm run build      # -> web/dist, embedded into the binary
cd web && npm run typecheck
cd web && npm run lint       # eslint; CI runs this too

python3 test/fixtures/verify_fixtures.py    # the binary fixtures are hand-built; this proves they are still valid
```

Running locally needs a `kbview.config.json` and at least one account; the server refuses
to start with no accounts:

```sh
./target/debug/kbview user add you@example.com --password-stdin <<< 'a-long-password'
./target/debug/kbview
RUST_LOG=kbview=debug ./target/debug/kbview     # see reindex/watch activity
```

## CI

`.github/workflows/ci.yml` runs the gate list above, then publishes a multi-arch image to
`ghcr.io/ziweiwu/personal-knowledge-base-webapp` on pushes to `main`.

The publish job **cross-compiles with `cargo-zigbuild`** and copies the binary into
`deploy/Dockerfile.ci`, which compiles nothing. Building Rust inside a multi-platform
image instead would run the non-native half under QEMU, where a release build with LTO
takes tens of minutes and can exhaust the runner. `deploy/Dockerfile` is the
build-from-source path and is not what CI uses.

CI also fails if `web/src/api/types.ts` changes during `cargo test` — see the boundary
note below.

## Architecture

One Rust binary serves the API, the embedded frontend, and file bytes. There is **no
database**. The reference corpus is ~1 MB of text across ~100 files, so the whole index
lives in memory and any change triggers a full rebuild in tens of milliseconds. Most of
the complexity a document app would normally carry is absent by that choice — if you find
yourself adding incremental indexing or a cache invalidation graph, check whether the
corpus actually grew first.

```
crates/kbview-core/    domain: paths, kinds, links, frontmatter, tasks, index, search, wire types
crates/kbview-docx/    OOXML -> HTML, deliberately isolated
crates/kbview-server/  axum, auth, rendering, watching, CLI
web/                   Vite + React + TS
```

### Things that are load-bearing

**`kbview_core::paths::resolve_in_root` is the security boundary.** Every filesystem read
and write goes through it. It canonicalises the nearest existing ancestor so a symlink
pointing out of the folder is caught even for a file that does not exist yet. Do not add a
filesystem access that bypasses it, and do not "simplify" the canonicalisation away.

**Root paths are canonicalised at config load.** Watch events always carry canonical
paths; a root configured through a symlink (`/tmp` on macOS) would never match its own
events and the index would silently go stale. This was a real bug, not a hypothetical.

**Saves carry `baseMtimeMs` and a mismatch is a 409.** Obsidian may have the same file
open. Removing the precondition means last-write-wins and silent data loss.

**Rename rewrites inbound links** using byte ranges from `scan_wikilinks`, which skips
code blocks and inline code. Never replace this with a regex over the document: the tests
in `crates/kbview-core/src/links.rs` cover prose that merely mentions the name, and links inside code samples,
because both get corrupted by naive replacement.

**A task checkbox's line number comes from the raw file, never from the parsed
document.** `kbview_core::tasks::task_lines` scans the source with the same scanner
`set_task_state` writes through, and `enable_task_checkboxes` pairs its results with the
rendered checkboxes. Do not go back to comrak's `sourcepos` for this: the parser sees a
*prepared* source with frontmatter stripped and callouts expanded, so its line numbers
drift from the file on disk and a click silently ticks a different task. The pairing is
checked on both count and ticked state; on any disagreement no checkbox is wired up at
all, because a dead checkbox is a smaller failure than one editing the wrong line.

**The auth gate is a `route_layer` on the whole `/api` group** in `crates/kbview-server/src/router.rs`, not
per-route. A new route is protected by construction. `crates/kbview-server/tests/api.rs`
asserts this for every route; add new routes to that list.

### The API's shape

Roots are addressed two ways and both are load-bearing: routes that act on a **path** take
it as a segment (`/api/doc/{root}/{path…}`), routes that act on the **root** take it as a
query parameter (`/api/tree?root=`, `/api/search?root=`, `/api/rename?root=`). Uploads are
raw bytes to `POST /api/file/{root}/{path…}`, deliberately separate from the JSON
`POST /api/doc/…` so neither has to infer its body shape from a content type.

Ticking a checkbox is `POST /api/task/{root}/{path…}`, deliberately not a save: it names a
line and a state, so it cannot carry content even if asked. It takes the same
`baseMtimeMs` precondition and answers a mismatch with the same 409 — as `AppError::Stale`,
which has no body, since a click has no edited buffer to offer back.

Mutating requests carry `X-Kbview-Origin`, echoed on the change event so the tab that made
a change ignores its own echo. `AppState` remembers the mtime it wrote and only attributes
the watcher's echo when the file still holds exactly that — matching on path alone would
swallow a genuine external edit landing in the same window, which is the one event the
author most needs to see.

### Rendering

Obsidian syntax (wikilinks, embeds, callouts, tags) is rewritten **in the source before
parsing**, reusing the code-aware scanners in `kbview-core`. Standard markdown is left to
comrak. This is why the link graph and the rendered output cannot disagree — they come
from the same scanner.

Maths renders to **MathML** via `latex2mathml`, so no client-side maths library ships.
`escape_stray_dollars` runs first and escapes any `$` that is not a delimiter under
Pandoc's rules, plus any `$` followed by a digit. Without it, "costs $5 today and $7
tomorrow" is parsed as maths and the prose between the two amounts is swallowed into a
MathML blob — silently, since the page still renders.
Syntax highlighting uses a custom syntect adapter emitting **CSS classes, not inline
colours** — comrak's bundled adapter writes `style="color:…"` which breaks dark mode.

Renderers degrade rather than fail: an unconvertible document sets `renderWarning` and
still displays. Preserve that; a blank pane with no explanation is worse than a partial
render with one.

### The Rust/TypeScript boundary

`web/src/api/types.ts` is **generated** by `ts-rs` during `cargo test`, directed by
`.cargo/config.toml`. Never hand-edit it. Numeric fields carry `#[ts(type = "number")]`
because `u64`/`i64` otherwise map to `bigint`, which `JSON.parse` never produces.

### View preferences

Sort, tree expansion and theme persist in `localStorage` through
`web/src/lib/persist.ts`. Every access is wrapped: storage *throws* in Safari private
browsing, and a lost preference must never become a blank page.

A folder's name filter is stored **per folder**, never globally — one folder's filter
silently hiding another folder's contents is the failure mode that keying it this way
prevents. A restored filter also shows "N of M" and a Clear button, so it can never look
like the folder is simply empty.

## Conventions

- Tests name the behaviour, not the function (`a_save_with_a_stale_base_mtime_is_refused`).
- **`cargo test` output must be checked for `FAILED`, not just summed.** Summing the
  per-suite pass counts hides a failing suite entirely; this has already produced one
  false "all green" report.
- Comments explain why a non-obvious choice was made; they do not restate the code.
- New document kinds: add to `crates/kbview-core/src/kinds.rs`, a renderer arm in `crates/kbview-server/src/render/document.rs`, and a
  viewer on the client. The three must stay in step.
