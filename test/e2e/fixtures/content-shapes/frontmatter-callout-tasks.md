---
title: Frontmatter, Callout, Tasks
tags: [regression]
---

# The line-offset shape

This document exists because of a real corruption bug. Frontmatter is stripped and the
callout below expands into HTML *before* the parser sees the source, so a checkbox line
number taken from the parser points somewhere else in the file on disk.

> [!warning] Do not simplify this fixture
> Remove the frontmatter, or the callout, and the shape stops reproducing.
> Both are needed, and so is a task list with a decoy far enough down.

- [ ] the first task, which is what a click on the first checkbox must tick
- [ ] filler one
- [ ] filler two
- [ ] filler three
- [ ] filler four
- [ ] filler five
- [ ] the decoy, which must never change unless it is clicked directly
