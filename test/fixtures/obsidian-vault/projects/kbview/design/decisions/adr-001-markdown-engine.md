---
title: "ADR 001: Markdown Engine"
tags:
  - kbview
  - adr
  - design
date: 2026-08-17
aliases:
  - ADR 001
  - Markdown Engine Decision
status: accepted
---

# ADR 001: Markdown Engine

This note sits four folders deep
(`projects/kbview/design/decisions/`) so folder-tree rendering and breadcrumb
generation get a non-trivial case.

## Context

We need CommonMark plus GFM plus Obsidian extensions. Writing a parser is not the
project. See [[projects/kbview/design/rendering-pipeline#Stages]] for where this
sits.

## Decision

Use a CommonMark parser with GFM enabled and add the Obsidian extensions as a
post-processing pass over the AST.

## Consequences

> [!tip] Upside
> Wikilinks, embeds and callouts stay in one place and can be switched off for
> non-vault roots.

> [!warning] Downside
> Two passes over the tree.
>
> > [!note] Nested inside the warning
> > Nested callouts must render as nested blocks, not as a flattened quote.
> > A deeper level still: 
> >
> > > [!danger] Third level
> > > If this collapses into the parent, the callout parser is wrong.

## Links back

- Up one: [[projects/kbview/design/rendering-pipeline]]
- Up two: [[projects/kbview/overview]]
- Root: [[index]]
