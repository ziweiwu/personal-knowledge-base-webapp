# kbview Handbook (plain Markdown fixture)

This folder is **not** an Obsidian vault. There is no `.obsidian/` directory
anywhere under it, and nothing here uses wikilink syntax. Everything is
plain CommonMark plus ordinary relative links.

There is deliberately **no `index.md` at this root**, so the app must fall back to
`README.md` as the landing page. (The fallback order is `index.md`, then
`README.md`, then a generated folder listing.)

## Contents

- [Guides](./guides/index.md) — a subfolder that *does* have an `index.md`.
- [Reference](./reference/) — a subfolder with **no** index file, so the app has
  to generate a listing for it.
- [Contributing](./contributing.md) — a sibling file at this root.
- [Changelog](./changelog.md) — another sibling.

## Relative links to exercise

| Link style | Example | Written as |
| --- | --- | --- |
| Same folder | [contributing](./contributing.md) | `./contributing.md` |
| Same folder, no dot | [changelog](changelog.md) | `changelog.md` |
| Into a subfolder | [installation guide](./guides/installation.md) | `./guides/installation.md` |
| Two levels down | [reverse proxy](./guides/advanced/reverse-proxy.md) | `./guides/advanced/reverse-proxy.md` |
| Folder, no file | [the reference folder](./reference/) | `./reference/` |
| With a fragment | [CLI flags](./reference/cli.md#flags) | `./reference/cli.md#flags` |
| Fragment only | [jump to contents](#contents) | `#contents` |
| Absolute URL | [example.com](https://example.com/kbview) | full URL |
| Broken on purpose | [missing page](./does-not-exist.md) | must not 500 |

## A relative image

Referenced from the root, so the path has no `..` in it:

![Architecture diagram](./assets/diagram.png)

And one written without the leading `./`:

![Screenshot of the reader](assets/screenshot.png)

## Plain CommonMark only

- Task list: - [x] this is GFM, and is fine
- Table: see above
- Footnote[^note]

[^note]: Footnotes are GFM, not Obsidian. Double-bracket wikilink syntax must not
    appear anywhere in this root — not even inside a code span, so that a test can
    assert its total absence. If the app turns bare text into a link here, it is
    applying vault rules to a non-vault root.
