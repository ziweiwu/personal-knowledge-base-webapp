---
title: Callouts
tags:
  - reference
  - kbview/markdown
date: 2026-08-19
aliases:
  - Admonitions
---

# Callouts

Obsidian callouts are blockquotes whose first line is `[!type]`, optionally
followed by a title.

## Untitled note callout

> [!note]
> A plain note callout with no title. The renderer supplies the word "Note".

## Titled warning

> [!warning] With A Title
> The title comes from the text after the type marker and replaces the default
> label. Body text can span
> multiple lines and contain **bold**, `code`, and [[Glossary|links]].

## Tip

> [!tip]
> Callout bodies can hold lists:
>
> - one
> - two
> - three
>
> and fenced code:
>
> ```sh
> kbview serve --root ./test/fixtures/obsidian-vault
> ```

## Nested

> [!info] Outer callout
> This is the outer body.
>
> > [!question] Inner callout
> > This one is nested one level in.
> >
> > > [!success] Innermost
> > > And this one is two levels in. All three must render as distinct boxes.
>
> Back to the outer body after the nested block.

## Foldable variants

> [!abstract]- Collapsed by default
> The trailing `-` means the callout starts folded. A `+` means it starts open.

> [!example]+ Expanded by default
> Body of an explicitly expanded callout.

## Not a callout

> This is an ordinary blockquote. It has no `[!type]` marker and must render as a
> plain quote, not as a callout box.
