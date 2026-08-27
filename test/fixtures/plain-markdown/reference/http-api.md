# HTTP API

[CLI reference](./cli.md) · [config schema](./config-schema.md) ·
[guides](../guides/index.md)

## Endpoints

| Method | Path | Returns |
| --- | --- | --- |
| `GET` | `/api/roots` | The configured roots. |
| `GET` | `/api/tree/{root}` | The folder tree for one root. |
| `GET` | `/api/doc/{root}/{path}` | Rendered HTML for one document. |
| `GET` | `/api/raw/{root}/{path}` | Raw bytes, for images and downloads. |
| `GET` | `/api/search/{root}?q=` | Full-text hits. |

## Error shape

```json
{
  "error": "not_found",
  "message": "no such document: reference/nope.md",
  "status": 404
}
```

A link to a file that does not exist, such as
[this one](./nope.md), should produce exactly that rather than a stack trace.
