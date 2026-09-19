# Invariants

Properties that must hold for kbviewer to be safe to leave running against the
user's real vault. Each is greppable from a test name: `cargo test INV-2` finds
nothing today, so the **Enforced by** line names the tests that already pin the
property under their own names.

Numbering is stable and never reused. Retire an entry in place rather than
renumbering.

The deployment these are written for: one Rust binary in an Alpine/musl
container on a Synology NAS (`kbview`, uid 1033, `mem_limit: 1g`), serving
`/vault`, which is a Synology Drive copy of the Obsidian vault the user edits on
a Mac. So there are always three writers to the same files (this app, Obsidian
through Drive sync, and Drive itself landing the other direction), the
container's uid is not the user's, the NAS filesystem reports inotify reads,
and the usual client is a phone over Tailscale.

Severity order: the entries that destroy or leak data come first.

## INV-1 — A request path never resolves outside its root, symlinks included

`kbviewer_core::paths::resolve_in_root` is the only way a request path becomes
a filesystem path. It rejects `..`, absolute paths and embedded NUL, then
canonicalises the nearest *existing* ancestor of the target so a symlink
anywhere on the path is followed and checked against the canonical root.
Roots themselves are canonicalised once at config load (`config.rs`). Every
read, write, delete, rename endpoint and the docx media route go through it.

**Why this matters here:** the vault is an SMB share. A symlink created on the
Mac resolves there and is a literal path on the NAS, and Drive can sync one
in. The container sees `/vault` only, but a symlink to `/etc` inside it would
still be inside the bind mount as far as a naive prefix check is concerned.
The write routes create parents (`create_dir_all`) so a path that does not
exist yet must be checked too, which is why the *nearest existing ancestor*
is what gets canonicalised.

**Enforced by:** `paths.rs` unit tests `rejects_parent_traversal`,
`rejects_absolute_paths`, `rejects_embedded_nul`,
`rejects_a_symlink_pointing_out_of_the_root`,
`accepts_a_path_that_does_not_exist_yet`; `tests/api.rs`
`traversal_attempts_are_refused` (encoded `..%2f`, `.obsidian/app.json`).

## INV-2 — A save never overwrites a change made elsewhere

`routes/write.rs::save` compares the file's current mtime (ms) with the
`baseMtimeMs` the client read the document at. Mismatch is `409 conflict`
carrying both texts (`SaveConflict { yourContent, diskContent, diskMtimeMs }`)
and writes nothing. The frontend must re-derive `baseMtimeMs` from every
document fetch and every save response, never from a clock.

**Why this matters here:** Obsidian on the Mac has the same note open. Drive
lands its save on the NAS with a new mtime, so the precondition is the only
thing standing between a phone edit and silently discarding the desktop one.
The window is not theoretical: sync latency is seconds, a note stays open in a
phone tab for hours.

The precondition is only as good as the gap between checking it and writing.
Since 2026-09-19 every write route holds the root's `AppState::write_gate` from
its check to its write; before that, two requests on different worker
threads both passed the same check and both wrote, so a `create` race gave
two 201s with one body on disk (measured, C-2), and two saves with the same
`baseMtimeMs` would have done the same to an edit.

**Enforced by:** `tests/api.rs`
`a_save_with_a_stale_base_mtime_is_refused_and_changes_nothing`,
`a_save_with_the_current_mtime_succeeds`,
`concurrent_creates_of_one_path_succeed_exactly_once`;
`web/e2e/editing.spec.ts` covers the 409 dialog. The QA pass of 2026-09-19
drove the frontend through a 409 and saw it adopt the disk version and retry
without looping (C-8).

## INV-3 — A checkbox toggle flips one character on a verified task line, from the file on disk

`routes/write.rs::toggle_task` re-reads the file, requires the same
`baseMtimeMs` precondition as a save (`409 stale` otherwise), and calls
`tasks::set_task_state`, which only ever replaces the character between `[`
and `]` on the addressed 1-based line, refuses (`NotATask`) if that line is no
longer a task or sits inside a fence, and is a no-op (`AlreadySet`) when the
state already holds. Line numbers come from `tasks::task_lines` over the raw
file, never from the markdown AST, because frontmatter and callouts shift
comrak's line numbers away from the file.

**Why this matters here:** ticking a box is the main thing the phone does.
Ticking the wrong line corrupts a different task with no diff to show for it,
and a document edited on the Mac between render and click is the normal case,
not the edge.

**Enforced by:** `tasks.rs` unit tests (quoted, nested, frontmatter offset,
fence skipped, CRLF preserved, CJK); fixtures
`frontmatter-callout-tasks.md`, `tasks.md`, `raw-html-checkbox.md`;
`web/e2e/tasks.spec.ts`.

## INV-4 — A checkbox is only wired when rendered boxes and raw task lines pair exactly

`render/markdown.rs` counts `<input type="checkbox">` in the output and pairs
them positionally with `task_lines` from the source. Any mismatch (a literal
`<input>` in the note, a task inside raw HTML, a shape one scanner sees and the
other does not) leaves *every* checkbox in that document read-only rather than
pairing them off by one.

**Why this matters here:** raw HTML is rendered (INV-18), so one pasted
`<input type="checkbox">` would otherwise shift every click below it by a line.

**Enforced by:** fixture `raw-html-checkbox.md` and `tasks.md` via
`web/e2e/tasks.spec.ts`; `markdown.rs` `each_checkbox_is_enabled_and_carries_its_line`,
`a_count_mismatch_leaves_every_checkbox_alone`,
`a_state_mismatch_leaves_every_checkbox_alone`.

## INV-5 — A rename rewrites every inbound wikilink, only in prose, and never repoints one at a different note

`routes/write.rs::rename` plans rewrites first from the *on-disk* text of each
backlinking document (not the index's cached body), moves the file, then
applies the rewrites. `links::rewrite_wikilinks` replaces only byte ranges that
`scan_wikilinks` found outside code spans and fences, and only when the new
name resolves to the renamed document in the corpus *after* the move
(`Resolver` built from relocated paths). Folder renames relocate sources that
moved with the folder. A source that fails to rewrite is logged and reported,
not fatal, because the move already happened.

**Why this matters here:** links are `[[note]]`, `[[folder/note|alias]]` and
`[[note#heading]]` across hundreds of notes, and the rewrite syncs to every
device within seconds. A wrong rewrite is indistinguishable from an edit.

**Enforced by:** `tests/api.rs` `renaming_rewrites_inbound_links`,
`renaming_a_folder_rewrites_path_qualified_inbound_links`,
`a_document_inside_a_renamed_folder_is_rewritten_at_its_new_path`,
`a_rename_never_repoints_a_link_at_a_different_document`,
`renaming_into_an_excluded_path_is_refused`; `links.rs` unit tests.

## INV-6 — Every write lands atomically and leaves no stray file the index would show

`write_atomic` (save, toggle, create, link rewrites) and `upload` write to a
`NamedTempFile` in the target's directory, `sync_all`, then `persist`
(rename). Readers, the watcher and Drive see the old file or the new one.
The temp name starts with `.tmp`, which `is_excluded` hides, so an aborted
write is invisible rather than indexed as a document.

**Why this matters here:** the directory is watched (400 ms debounce) and
synced. A truncated note synced to the Mac is a lost note.

**Enforced by:** none as a test. The fence is structural (one helper). An
aborted-write test does not exist.

## INV-7 — A file written by kbviewer keeps an owner and mode the other writers can use

`write_atomic` copies the existing file's mode onto the temp file before
`persist` replaces the inode, and gives a file it creates 0644; uploads go
through the same helper. Until 2026-09-19 nothing did, so every saved note
came back 0600 owned by the container uid (1033:65536) — measured on the QA
fixture, C-1. `store.rs` sets 0600 deliberately for `users.json` and
`sessions.json`; that is the one place a private file is right.

**Why this matters here:** Drive runs as the user's DSM account, not uid
1033. A 0600 file owned by another uid may not sync back, or may sync as a
conflict copy, and the Mac side then shows the pre-save text. Whether that
happens depends on the share's ACL mode, which is why this needs measuring
rather than reading.

**Enforced by:** `write.rs`
`an_atomic_write_keeps_the_mode_the_file_already_had`,
`a_file_written_for_the_first_time_is_readable_by_others`; `tests/api.rs`
`a_save_keeps_the_files_mode_and_a_new_file_is_not_private`. Whether Drive
syncs a 0600 file back was never measured on the NAS; the fix removes the
question rather than answering it.

## INV-8 — Every `/api` route except login requires a session, and file bytes are never public

`router.rs` puts every protected route under one `route_layer` running the
session middleware; only `POST /api/auth/login` is outside it. Static assets
and the SPA shell are public. Adding a route to the protected router is the
only way to add one, so a new unauthenticated route has to be a deliberate act.

**Why this matters here:** the NAS port is reachable from the LAN and over
Tailscale, and `/api/file` serves the vault's attachments byte for byte.

**Enforced by:** `tests/api.rs` `every_api_route_requires_a_session` (asserts
ten routes; `/api/task`, `/api/rename`, `/api/tag`, `/api/docx-media` and the
write verbs are covered structurally but not listed), `file_bytes_are_not_public`,
`a_session_unlocks_the_api`, `logging_out_invalidates_the_session`.

## INV-9 — A cross-site browser request cannot mutate anything

`auth/middleware.rs` refuses any mutating method whose `Origin` host differs
from the `Host` it arrived at (`403`). A request with no `Origin` is accepted
because it is not a browser. The session cookie is `SameSite=Lax`, `HttpOnly`,
and `Secure` when a forwarded header says the hop was HTTPS.

**Why this matters here:** the phone reaches the app at `my-nas:4321` and the
Mac at `127.0.0.1:4321`; both are same-origin with themselves. A reverse proxy
that rewrites `Host` would turn every browser write into a 403, which is a
visible failure and the intended one.

**Enforced by:** `tests/api.rs` `a_cross_origin_write_is_refused`;
`middleware.rs` unit tests (same host, different host, no Origin,
`kb.example.ts.net`).

## INV-10 — A refusal never explains itself

Traversal, an excluded path and a missing file are all `404 not_found` with
the same body. A wrong password and an unknown email are the same `401`.
`error.rs` maps every `AppError` to a fixed `(status, code)` pair and the
message never contains the path or the reason a path was rejected.

**Enforced by:** `error.rs` unit tests; `tests/api.rs`
`a_wrong_password_and_an_unknown_email_are_indistinguishable`,
`traversal_attempts_are_refused` (asserts no `root:` leaks),
`an_unknown_api_path_returns_json_not_the_html_shell`.

## INV-11 — Excluded paths are never indexed, served, or written to

`paths::is_excluded` names `.obsidian`, `.trash`, `.git`, `.svn`, `@eaDir`,
`.SynologyWorkingDirectory`, `node_modules`, macOS `Icon\r` files and any
dot-prefixed component. The indexer, the
watcher (`relative_if_relevant`), the file routes and every write
(`reject_excluded` on create, create_folder, upload, delete, and on both ends
of a rename) agree on it. A save cannot reach one because it must already be
in the index. Delete and a rename's *source* were unchecked until
2026-09-19: `DELETE .obsidian` moved the whole Obsidian configuration into
`.trash/`, and renaming `.obsidian/app.json` into the open served a file the
index hides.

**Why this matters here:** `@eaDir` is the DSM thumbnail tree that is
invisible over SMB and present on the NAS; `.obsidian/` holds workspace state
Obsidian rewrites constantly and would otherwise trigger reindexes and appear
as documents. Writing into an excluded path succeeds on disk and then
vanishes, which is worse than refusing.

**Enforced by:** `paths.rs` `excludes_sync_and_tool_artefacts`; `watch.rs`
`ignores_changes_in_excluded_directories`; `tests/api.rs`
`renaming_into_an_excluded_path_is_refused`,
`excluded_paths_can_be_neither_deleted_nor_renamed_out`,
`traversal_attempts_are_refused`.

## INV-12 — Delete is recoverable and never clobbers an earlier deletion

`routes/write.rs::delete` moves the path into `<root>/.trash/` preserving its
folder layout, matching Obsidian's default. A second delete of the same path
lands as `name (1).ext`, `name (2).ext` and so on, never over the first.

**Why this matters here:** deleting from a phone over Tailscale has no undo
and no confirmation dialog worth the name.

**Enforced by:** `tests/api.rs` `deleting_moves_to_trash_rather_than_destroying`.
The `(n)` disambiguation: `write.rs`
`deleting_the_same_path_twice_does_not_overwrite_the_first`.

## INV-13 — A `readOnly` root refuses every write with `403 read_only`

`writable_root` is the first line of every write handler and is the only place
`read_only` is consulted on the request path.

**Enforced by:** none at the route level. `tests/api.rs` contains no
`read_only` case. Candidate: one missing `writable_root` call is invisible.

## INV-14 — Login is rate limited and the limiter's memory is bounded

`auth/rate_limit.rs` counts failures per email and per address in a sliding
window and locks out past a threshold. Every recorded failure prunes entries
older than the window first, so the map cannot grow past what one window of
distinct failing keys holds.

**Why this matters here:** Argon2id makes each attempt expensive by design;
the container has 1 GiB and the port is reachable by anything on the tailnet.

**Enforced by:** `rate_limit.rs` unit tests including
`a_new_failure_drops_entries_that_aged_out_of_the_window`.

## INV-15 — Accounts and sessions survive restart, die with their account, and are 0600

`auth/store.rs` writes `users.json` and `sessions.json` atomically with mode
0600. Sessions persist across a restart. Deleting an account through the CLI
removes its sessions. Accounts are read **once at startup**: an account added
by the CLI while the server runs is not visible until restart (documented in
`macos/README.md`).

**Enforced by:** `store.rs` unit tests (persistence, cascade delete, mode).

## INV-16 — Every document reaches a rendered or explained state, never a blank pane

`render/` never returns nothing: an empty file, an opaque binary, a docx with
a missing part or an oversized zip entry, a CSV with ragged rows or an
unclosed quote, a frontmatter block that never closes, and CJK inside a fence
each produce either a rendering or the source with a reason. `kbviewer-docx`
caps part size to refuse zip bombs and never panics on bad XML. A text-kind
file whose bytes are not UTF-8 (a Latin-1 note, an upload whose extension
lies) is indexed with no content, reported as not editable, and rendered as a
warning naming the encoding; its raw route answers 400, not 404. Before
2026-09-19 it was a blank pane and an editor that said the file was gone.

**Why this matters here:** 1 GiB and a single process. One panic or one
runaway allocation on one odd file takes the whole viewer down for every tab.

**Enforced by:** fixtures `empty.md`, `opaque.bin`, `meeting-minutes.docx`,
`inventory.csv`, `code-and-highlighting.md`; `text.rs`, `frontmatter.rs`,
`package.rs` unit tests; `web/e2e/formats.spec.ts`, `rendering.spec.ts`;
`test/fixtures/verify_fixtures.py` for fixture shape; `tests/api.rs`
`a_text_file_that_is_not_utf8_is_explained_rather_than_lost`.

## INV-17 — A write preserves line endings and the trailing-newline state byte for byte

A save round-trips the editor text unchanged; a toggle uses
`split_inclusive('\n')` so `\r\n` survives; nothing normalises on the way out.

**Enforced by:** `tasks.rs` CRLF test; fixtures `crlf.md`,
`no-trailing-newline.md`; `crlf.md` via `web/e2e/tasks.spec.ts` ("a task in a
CRLF document" asserts no bare `\n` after a toggle). No e2e spec names
`no-trailing-newline.md`.

## INV-18 — Every string interpolated into generated HTML is escaped; raw HTML in a note is accepted

`render/html.rs` escaping is applied to every title, path, anchor and
attribute the renderer builds. Comrak runs with `render.unsafe = true` on
purpose: the notes are the authenticated user's own files and Obsidian renders
their inline HTML too (README accepted-risk note).

**Enforced by:** `html.rs` unit tests. Do not report raw HTML in notes as a
finding.

## INV-19 — Obsidian mode is decided once per root, logged, and warned about when it looks wrong

A root uses wikilinks iff `.obsidian/` exists or the config sets `wikilinks`.
Startup logs `indexed root ... wikilinks=<bool>`; a root with no `.obsidian/`,
no override, and notes containing `[[...]]` outside code logs a warning.
Rename link rewriting is skipped entirely for non-wikilink roots.

**Why this matters here:** Synology Drive does not sync dot-folders, so the
NAS copy has no `.obsidian/` and ran as a plain folder for a week without
anyone noticing (every link rendered as text).

**Enforced by:** `index.rs` `notes_with_wikilink_syntax` tests; `tests/api.rs`
`a_session_unlocks_the_api` asserts `obsidianMode:true` for the vault fixture.

## INV-20 — Every other tab learns of a change; the writer's own tab never fights its echo

Writes reindex synchronously and broadcast a `ChangeEvent` on SSE. A mutating
request carries `X-Kbviewer-Origin`; the reindex records it against the
file's mtime so the watcher's later event for the same write is attributed to
that origin and the originating tab ignores it. A lagging SSE receiver has its
lag error dropped and stays connected; it catches up on the next event it
sees. A 401 on the stream triggers a full reload.

**Why this matters here:** the editor on the Mac and the viewer on the phone
are open at the same time all day; a self-echo would reload the editor under
the user's cursor.

The header name is the whole contract and neither side can check the other
at compile time. From the `kbview` → `kbviewer` rename until 2026-09-19 the
client sent `X-Kbview-Origin` and the server read `x-kbviewer-origin`, so no
browser write ever carried an origin and every tab fought its own echo; the
`state.rs` tests passed throughout because they feed the origin past the HTTP
layer.

**Enforced by:** `state.rs` unit tests (author not warned about own save);
`watch.rs` `ignores_reads`, `reports_writes`; `tests/api.rs`
`a_browser_write_carries_its_origin_onto_the_change_event` and
`the_client_sends_the_origin_header_the_server_reads`, which reads the
TypeScript constant. No test covers what a tab shows after more than 256
events were dropped; see Candidates.

## INV-21 — A read never triggers a reindex

`watch.rs::is_change` accepts `Access(Close(Write))` and rejects every other
`Access` event. Everything else (create, modify, remove, rename) reindexes the
whole root, logged at `info` as `reindexing after change` with a count.

**Why this matters here:** Linux inotify reports `IN_OPEN`/`IN_CLOSE_NOWRITE`
for the indexer's own reads. Before the filter the container rebuilt the index
in a loop and hit `mem_limit: 1g` 56 times a day. The `info` line is the only
visible sign of a recurrence.

**Enforced by:** `watch.rs` `ignores_reads`, `reports_writes`; the
`reindexing after change` count in the container log on an idle NAS should be
zero.

## INV-22 — Image variants are whitelisted widths, never wider than the original, never stale

`/api/file/...?w=N` serves a resized variant only for `N` in
`[400, 800, 1200, 1600]` and only when `N` is smaller than the source; any
other query serves the original bytes. Variants are cached and invalidated
when the source's mtime changes, and the cache has a byte budget.

**Enforced by:** `tests/api.rs`
`a_resized_variant_is_smaller_than_the_original_and_cached`,
`a_variant_is_never_larger_than_the_original`,
`a_width_that_is_not_offered_serves_the_original_untouched`; `variants.rs`
unit tests.

## INV-23 — The frontend's request shapes are the backend's

`web/src/api/types.ts` is generated by ts-rs during `cargo test` and never
hand-edited. `client.ts` sends `SaveRequest`, `TaskToggleRequest`,
`RenameRequest` as typed payloads (`satisfies`). A drift fails CI on
`git diff --exit-code -- web/src/api/types.ts` and on `tsc --noEmit`.

**Enforced by:** CI (`.github/workflows/ci.yml`).

## INV-24 — Cargo and the embedded frontend agree on what is being served

The binary embeds `web/dist` at compile time (rust-embed). A `cargo build`
after a frontend change without `npm run build` ships the old UI against the
new API with no error anywhere. `run-server.sh` refuses to start without
`web/dist/index.html`, and asset cache headers mark hashed files immutable
but never `index.html`.

**Enforced by:** `assets.rs` unit tests; CI builds web before cargo.

---

## Candidates

Ungrounded or unenforced properties that a QA pass should settle rather than
assume. Each names the one check that decides it.

Settled on 2026-09-19 by the first QA pass and fixed the same day: C-1
(0600 confirmed on the fixture, mode now preserved), C-2 (two 201s
confirmed, writes now serialised), C-4 (the rename dialog promised rewrites
in every root; it now says so only in Obsidian mode), C-6 (all six write
verbs answer `403 read_only`, no defect), C-8 (the editor recovers from a
409, no defect), C-10 (clean not-found state, no defect), C-11 (500
confirmed, now a 400 before anything touches disk).

- **C-3 (INV-5) Link rewrites carry no mtime precondition.** A backlinking
  document changed between planning and `apply_link_rewrites` is overwritten
  with the planned text. Window is milliseconds, but Drive lands files
  whenever it likes. Question: does `write_atomic` for a rewrite compare the
  mtime read during planning? (It does not; the question is whether that is
  acceptable and documented.)
- **C-5 (INV-8) The per-route auth test lists ten of the routes.** Add the
  missing ones or accept that the `route_layer` is the oracle. Question: does
  `every_api_route_requires_a_session` fail if a route is added *outside*
  `protected_routes()`? (No. It only checks the ones it names.)
- **C-6 (INV-13) `readOnly` has no integration test.** Verified by hand on
  2026-09-19 (all six verbs 403); still needs a `harness_plain()` /
  read-only harness in `tests/api.rs`, which also unblocks INV-19's
  non-wikilink branch — `harness()` creates `.obsidian/` unconditionally.
- **C-7 (INV-20) A tab that missed more than 256 events stays stale until the
  next event.** A Drive burst (initial sync, a folder move) exceeds the channel
  capacity. Question: what does the open document show after such a burst and
  no further change?
- **C-9 (INV-9) `Secure` cookie behind Tailscale Serve.** `is_https` reads a
  forwarded header. If the tailnet HTTPS hop does not send one, the cookie is
  sent without `Secure` over HTTPS, which is harmless; if it sends one and the
  LAN path is plain HTTP on the same hostname, the browser drops the cookie
  and every LAN login appears to fail. Question: what does the actual NAS
  setup send?

## What has no oracle

Surfaces where nothing above asserts anything. This is the agenda for the
next round and the honest answer to a fuzz sweep that comes back clean.

- File owner and mode after any write on the NAS (C-1).
- Concurrency between two browser clients writing the same path (C-2, C-3).
  Every write test is single-client.
- The frontend's handling of every non-2xx write outcome: 409 conflict, 409
  stale, 409 already_exists, 403 read_only, 403 cross-origin, 413. Only the
  stale-save path has an e2e spec (`editing.spec.ts`, "a save against a stale
  base mtime is a 409 carrying both versions").
- SSE lag and reconnect behaviour beyond a 401.
- Memory over time under a real vault: the only measurement is 7 MiB idle
  after the inotify fix. No test bounds index size, search memory or the image
  variant budget end to end.
- The phone: the e2e phone projects are WebKit emulation; no real iOS Safari
  run exists, and Tailscale-path behaviour (cookie, Origin) is only reasoned.
- Search relevance and tag pages: `tests/api.rs` has no search or `/api/tag`
  assertions beyond authentication.
- The `mermaid.md`, `math.md`, `callouts.md` fixtures pin rendering shape but
  not fidelity; nothing asserts a diagram actually drew.
- Upload of a file whose extension lies (a `.png` that is a PDF, a `.md` that
  is binary): `kinds.rs` classifies by extension alone.
