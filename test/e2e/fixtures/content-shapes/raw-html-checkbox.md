# A checkbox the scanner cannot see

The input below is raw HTML, not a task list item. It gives the rendered page one more
checkbox than the source has task lines, which is exactly the disagreement that must make
the whole document's checkboxes read-only rather than pair them off by one.

<p><input type="checkbox" /> not a task, just markup</p>

- [ ] a real task that must stay read-only while the document is in this state
- [ ] a second real task
