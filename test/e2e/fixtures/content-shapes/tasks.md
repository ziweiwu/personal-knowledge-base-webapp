# Task shapes

The renderer draws a checkbox for each of these, and the writer must accept the same set.
Where the two disagree the document degrades to read-only, so a mismatch here is a bug.

- [ ] a plain unchecked task
- [x] a plain checked task
* [ ] a star marker
+ [ ] a plus marker

1. [ ] an ordered task with a dot
2) [x] an ordered task with a parenthesis

- [ ] a parent task
  - [ ] an indented child
  - [x] a checked child

> - [ ] a task inside a blockquote

Not tasks, and no checkbox may be drawn for any of them:

- [] empty brackets
- [y] not a state character
-[ ] no space after the marker

A task inside a fence is a code sample. It draws no checkbox, and the API must refuse a
request that names its line:

```markdown
- [ ] this line lives inside a code fence and must never be editable
```
