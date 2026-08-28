# Code

Highlighted server-side into CSS classes, never inline colours — inline colours are what
broke dark mode.

```rust
pub fn resolve_in_root(root: &Path, relative: &str) -> Result<PathBuf, PathError> {
    // The security boundary. Every filesystem access in the app goes through here.
    let normalised = normalise(relative)?;
    canonical_ancestor(root, &normalised)
}
```

```python
def escape_stray_dollars(source: str) -> str:
    """A `$` that is not a delimiter has to stop looking like one."""
    return source
```

```json
{ "id": "shapes", "name": "Content Shapes", "path": "/tmp/kbview-e2e/roots/content-shapes" }
```

A fence with no language at all:

```
plain preformatted text
```

CJK inside a fence. Stepping through this one byte at a time panicked the renderer, because
the caller then sliced mid-character:

```text
汉字と日本語のテキスト、絵文字も 🚀 含む
```

A line long enough that the block must scroll inside itself rather than the page:

```text
this single line is deliberately far too long to fit inside a phone viewport and exists purely so the surrounding block gets its own horizontal scroll container instead of pushing the document sideways
```

Inline `code` in a sentence, and a wikilink inside code that must NOT resolve: `[[tasks]]`.
