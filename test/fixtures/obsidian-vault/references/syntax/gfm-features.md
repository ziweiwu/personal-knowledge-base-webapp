---
title: GFM Features
tags:
  - reference
  - kbview/markdown
  - gfm
date: 2026-08-19
aliases:
  - GitHub Flavoured Markdown
---

# GFM Features

## Tables

Alignment markers must survive the round trip.

| Feature | Spec | Supported | Notes |
| :--- | :---: | ---: | --- |
| Tables | GFM | yes | including alignment |
| Task lists | GFM | yes | checked and unchecked |
| Strikethrough | GFM | yes | `~~like this~~` |
| Footnotes | GFM | yes | see below |
| Autolinks | GFM | yes | https://example.com/kbview |

A table cell containing a pipe: `a \| b`. A cell containing a wikilink:
[[Glossary]].

## Task lists

- [x] Parse frontmatter
- [x] Render tables
- [ ] Render footnotes
- [ ] Nested task lists
  - [x] child done
  - [ ] child not done
    - [ ] grandchild

## Strikethrough

The old plan was ~~to write our own parser~~ to reuse an existing one.

## Footnotes

kbview reads a folder and serves it[^1]. It does not index remote content[^remote].

[^1]: The folder is called a *root*. See [[Glossary]].
[^remote]: Deliberately out of scope for the first version.

## Autolinks and raw URLs

Bare URL: https://example.com/kbview/docs
Angle-bracket autolink: <https://example.com/kbview/spec>
Email: <hello@example.com>

## Tags, real and fake

A real inline tag: #kbview/markdown — this one belongs in the tag index.
Another real one: #gfm.

### Hex colours like #ffffff and #7c5cff are not tags

The heading above contains `#ffffff` and `#7c5cff`. Neither is a tag, and neither
is the leading `###` that makes it a heading.

The next block is fenced, so nothing in it is a tag:

```python
# TODO: #not-a-tag inside a python comment
COLOURS = {"accent": "#7c5cff", "bg": "#ffffff"}
tags = ["#neither-is-this"]
```

Inline code is exempt as well: `#nor-this-one`.

A number sign followed by a digit is not a tag either: issue #42 and #1234.

## Horizontal rules and line breaks

---

Line one ends with two spaces  
and line two follows a hard break.

## Nested lists with mixed markers

1. First
   - bullet child
     1. numbered grandchild
2. Second

   A loose paragraph inside the list item.

3. Third
