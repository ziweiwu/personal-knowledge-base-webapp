# Tables

A narrow table:

| Kind | Editable |
|---|---|
| markdown | yes |
| pdf | no |

A table wider than any phone. It must scroll inside its own container, and the page must
never scroll sideways:

| Route | Method | Body | Precondition | Success | Conflict | Notes |
|---|---|---|---|---|---|---|
| `/api/doc/{root}/{path}` | GET | none | none | 200 payload | n/a | html, meta, backlinks |
| `/api/doc/{root}/{path}` | PUT | JSON | `baseMtimeMs` | 200 meta | 409 both versions | atomic write |
| `/api/task/{root}/{path}` | POST | JSON | `baseMtimeMs` | 200 meta | 409 stale | one character only |
| `/api/file/{root}/{path}` | POST | bytes | none | 201 meta | 409 exists | upload |
| `/api/rename` | POST | JSON | none | 200 updated | 409 exists | rewrites inbound links |
