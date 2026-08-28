---
title: Content Shapes
tags: [e2e, fixtures]
---

# Content Shapes

Every document in this folder exists to pin one rendering or editing behaviour that has
broken at least once. Nothing here is decorative.

- [[frontmatter-callout-tasks]] — the line-offset regression
- [[tasks]] — every task-list shape the scanner must agree with comrak about
- [[raw-html-checkbox]] — a literal checkbox that must disable the whole document's boxes
- [[currency]] — prose that a permissive maths scanner eats
- [[math]] — MathML, and an environment that must degrade rather than fail
- [[code-and-highlighting]] — fences, long lines, and CJK inside a fence
- [[tables]] — a table wider than any phone
- [[callouts]] — Obsidian callout shapes
- [[links]] — resolved, unresolved, aliased, anchored
- [[mermaid]] — a diagram rendered on the client
- [[long-document]] — enough headings to exercise the table of contents
- [[Spaces And Caps]] — a filename with spaces and capitals
- [[unicode-标题]] — a non-ASCII filename
- [[crlf]] — CRLF line endings that a write must preserve
- [[empty]] — a zero-byte document
- [[no-trailing-newline]] — a last line with no newline after it
- [[deep/nested/folder/leaf]] — a path deep enough to test breadcrumbs

A link that goes nowhere: [[this-page-does-not-exist]].
