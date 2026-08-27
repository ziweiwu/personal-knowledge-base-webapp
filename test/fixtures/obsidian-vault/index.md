---
title: kbview Test Vault
tags:
  - vault/root
  - kbview
  - fixture
date: 2026-08-24
aliases:
  - Home
  - Vault Home
author: sam-example
publish: true
---

# kbview Test Vault

This is the landing page for the Obsidian-flavoured fixture vault. The app should
pick this file up as the root folder's landing page because it is named `index.md`.

## Where to start

- [[projects/kbview/overview|Project overview]] — what we are building.
- [[references/syntax/callouts]] — every callout shape the renderer must handle.
- [[references/syntax/gfm-features]] — tables, task lists, footnotes, strikethrough.
- [[references/syntax/math-and-diagrams]] — KaTeX and mermaid.
- [[Glossary]] — shared vocabulary, transcluded into a few other notes.
- [[Reading List]] — a note whose filename contains spaces.
- [[references/知识管理]] — a note with a CJK filename.

## Ambiguity on purpose

There are two notes called `Meeting Notes.md` in this vault:

1. `meetings/Meeting Notes.md`
2. `projects/kbview/notes/Meeting Notes.md`

The bare link [[Meeting Notes]] must resolve to the **shortest path**, i.e.
`meetings/Meeting Notes.md`. The explicit link
[[projects/kbview/notes/Meeting Notes|the project-scoped one]] must resolve to the
other file. See [[Meeting Notes#Decisions]] for a heading link into the winner.

## A link that must fail

This link points at nothing: [[Does Not Exist]]. It must render as an *unresolved*
link (Obsidian styles these differently and does not navigate on click) rather
than 404-ing the page or silently dropping the text.

## Attachments

An embedded image:

![[image.png]]

An embedded PDF:

![[document.pdf]]

## Tags

Inline tags live in prose too: this vault is tagged #kbview/fixture and
#status/active.

<!-- The tags below must NOT be extracted, see references/syntax/gfm-features. -->
