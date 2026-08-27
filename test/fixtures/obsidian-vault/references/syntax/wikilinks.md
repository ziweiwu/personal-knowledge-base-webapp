---
title: Wikilink Forms
tags:
  - reference
  - kbview/markdown
date: 2026-08-19
aliases:
  - Links
  - Wikilinks
---

# Wikilink Forms

Every wikilink shape the resolver must handle, in one place. This note is the
single-file target for link-resolution tests.

## Plain link

A bare basename: [[Glossary]].

## Aliased link

Pipe syntax, where the display text differs from the target:
[[Glossary|our shared vocabulary]].

## Link to a heading in another note

[[projects/kbview/design/rendering-pipeline#Stages]] should scroll to the
`Stages` heading of that note. A heading with spaces works too:
[[Meeting Notes#Follow-ups]].

## Link to a heading in this note

[[#Plain link]] jumps within this file. So does [[#Unresolved link]] further down.

## Link with a path

[[projects/kbview/notes/Meeting Notes|the design-review meeting]] disambiguates
the duplicated basename by spelling out the path.

## Unresolved link

[[Does Not Exist]] has no target anywhere in the vault. It must render as an
unresolved link: visible text, no navigation, no server error.

An aliased unresolved link is a second case:
[[Also Missing|this one is missing too]].

## Embeds are wikilinks too

- Image: `![[image.png]]`
- PDF: `![[document.pdf]]`
- Note transclusion: `![[Glossary]]`

Live examples of all three live in [[index]] and [[projects/kbview/overview]].
