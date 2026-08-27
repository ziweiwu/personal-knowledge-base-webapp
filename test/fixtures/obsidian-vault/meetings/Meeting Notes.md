---
title: Meeting Notes (weekly sync)
tags:
  - meeting
  - fixture/ambiguous-basename
date: 2026-08-24
aliases:
  - Weekly Sync
---

# Meeting Notes

> [!info] Disambiguation
> This is `meetings/Meeting Notes.md`. It has the **shortest path** of the two
> notes with this basename, so a bare `[[Meeting Notes]]` link must land here.
> The other one is [[projects/kbview/notes/Meeting Notes|under projects/kbview/notes]].

## Attendees

- sam-example
- the renderer
- the link resolver

## Decisions

1. Bare wikilinks resolve by shortest path when a basename is ambiguous.
2. Unresolved links render inert rather than erroring.
3. Attachment embeds are served from `attachments/`.

## Follow-ups

- [x] Write the fixture vault
- [ ] Wire the fixture into the integration tests
- [ ] Check [[Does Not Exist]] renders as unresolved

Jump back to [[#Attendees]] with a local heading link.
